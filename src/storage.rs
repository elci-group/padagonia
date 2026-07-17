//! On-disk storage format: file header, blocks, checksums, and parallel save/load.

use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::id::{LabelId, RelationId};
use crate::node::Node;
use crate::ontology::{StringTable, StringTableExt};
use crate::provenance::Provenance;
use crate::store::Store;
use ahash::AHashMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use thiserror::Error;

const MAGIC: &[u8; 8] = b"PADAGON\n";
const VERSION: u8 = 2;
const MAX_FRAME_BYTES: u64 = 1 << 30;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("MessagePack encode error: {0}")]
    MessagePackEncode(#[from] rmp_serde::encode::Error),
    #[error("MessagePack decode error: {0}")]
    MessagePackDecode(#[from] rmp_serde::decode::Error),
    #[error("CRC mismatch in block {block_index}")]
    CrcMismatch { block_index: usize },
    #[error("Bad magic or version")]
    BadHeader,
    #[error("Frame too large: {len} bytes")]
    FrameTooLarge { len: u64 },
    #[error("Trailing data after expected blocks: {bytes} bytes")]
    TrailingBytes { bytes: usize },
    #[error("Block kind does not match payload")]
    InconsistentBlockPayload,
    #[error("Unknown ontology string id {id}")]
    UnknownStringId { id: u32 },
    #[error("Dangling edge {edge_id} references missing node")]
    DanglingEdge { edge_id: u64 },
    #[error("Dangling fact references missing subject")]
    DanglingFact,
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockKind {
    Nodes(LabelId),
    Edges(RelationId),
    Facts,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BlockPayload {
    Nodes {
        label: LabelId,
        nodes: Vec<Node>,
    },
    Edges {
        relation: RelationId,
        edges: Vec<Edge>,
    },
    Facts {
        entries: Vec<(FactSubject, Provenance)>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub kind: BlockKind,
    pub payload: Vec<u8>,
    pub checksum: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileHeader {
    pub magic: [u8; 8],
    pub version: u8,
    pub string_table: StringTable,
    pub block_count: u64,
}

impl Store {
    /// Partition nodes by label and edges by relation, encode blocks in parallel, and write
    /// the file sequentially: header, then blocks.
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut node_partitions: AHashMap<LabelId, Vec<&Node>> = AHashMap::new();
        for node in self.nodes.values() {
            node_partitions.entry(node.label).or_default().push(node);
        }
        let mut edge_partitions: AHashMap<RelationId, Vec<&Edge>> = AHashMap::new();
        for edge in self.edges.values() {
            edge_partitions.entry(edge.label).or_default().push(edge);
        }

        let mut node_blocks: Vec<_> = node_partitions.into_iter().collect();
        let mut edge_blocks: Vec<_> = edge_partitions.into_iter().collect();
        node_blocks.sort_by_key(|(k, _)| k.0);
        edge_blocks.sort_by_key(|(k, _)| k.0);

        let node_payloads: Vec<_> = node_blocks
            .par_iter()
            .map(|(label, nodes)| {
                let payload = BlockPayload::Nodes {
                    label: *label,
                    nodes: nodes.iter().map(|n| (*n).clone()).collect(),
                };
                let bytes = rmp_serde::to_vec(&payload)?;
                Ok::<_, StoreError>((payload, bytes))
            })
            .collect::<Result<Vec<_>>>()?;

        let edge_payloads: Vec<_> = edge_blocks
            .par_iter()
            .map(|(rel, edges)| {
                let payload = BlockPayload::Edges {
                    relation: *rel,
                    edges: edges.iter().map(|e| (*e).clone()).collect(),
                };
                let bytes = rmp_serde::to_vec(&payload)?;
                Ok::<_, StoreError>((payload, bytes))
            })
            .collect::<Result<Vec<_>>>()?;

        // Persist only competing facts beyond the canonical one stored with the node/edge.
        let fact_entries: Vec<_> = self
            .facts
            .iter()
            .filter(|(_, facts)| facts.len() > 1)
            .flat_map(|(subject, facts)| facts[1..].iter().map(move |p| (*subject, p.clone())))
            .collect();
        let fact_bytes = rmp_serde::to_vec(&BlockPayload::Facts {
            entries: fact_entries,
        })?;

        let mut blocks = Vec::with_capacity(node_payloads.len() + edge_payloads.len() + 1);
        for ((label, _), (_, bytes)) in node_blocks.iter().zip(node_payloads.iter()) {
            blocks.push(Block {
                kind: BlockKind::Nodes(*label),
                payload: bytes.clone(),
                checksum: crc32fast::hash(bytes),
            });
        }
        for ((rel, _), (_, bytes)) in edge_blocks.iter().zip(edge_payloads.iter()) {
            blocks.push(Block {
                kind: BlockKind::Edges(*rel),
                payload: bytes.clone(),
                checksum: crc32fast::hash(bytes),
            });
        }
        blocks.push(Block {
            kind: BlockKind::Facts,
            payload: fact_bytes.clone(),
            checksum: crc32fast::hash(&fact_bytes),
        });

        let header = FileHeader {
            magic: *MAGIC,
            version: VERSION,
            string_table: self.string_table.clone(),
            block_count: blocks.len() as u64,
        };

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        write_frame(&mut writer, &header)?;
        for block in &blocks {
            write_frame(&mut writer, block)?;
        }
        writer.flush()?;
        Ok(())
    }

    /// Decode blocks in parallel and rebuild indexes.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::load_internal(path, true)
    }

    /// Decode blocks sequentially (for benchmarking the benefit of parallelism).
    pub fn load_seq<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::load_internal(path, false)
    }

    fn load_internal<P: AsRef<Path>>(path: P, parallel: bool) -> Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let header: FileHeader = read_frame(&mut reader)?;
        if &header.magic != MAGIC || header.version != VERSION {
            return Err(StoreError::BadHeader);
        }

        let mut raw_blocks = Vec::with_capacity(header.block_count as usize);
        for _ in 0..header.block_count {
            let block: Block = read_frame(&mut reader)?;
            raw_blocks.push(block);
        }
        let mut trailing = Vec::new();
        reader.read_to_end(&mut trailing)?;
        if !trailing.is_empty() {
            return Err(StoreError::TrailingBytes {
                bytes: trailing.len(),
            });
        }

        let decoded: Vec<(BlockKind, BlockPayload)> = if parallel {
            raw_blocks
                .into_par_iter()
                .enumerate()
                .map(|(idx, block)| {
                    if crc32fast::hash(&block.payload) != block.checksum {
                        return Err(StoreError::CrcMismatch { block_index: idx });
                    }
                    let payload: BlockPayload = rmp_serde::from_slice(&block.payload)?;
                    Ok((block.kind, payload))
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            raw_blocks
                .into_iter()
                .enumerate()
                .map(|(idx, block)| {
                    if crc32fast::hash(&block.payload) != block.checksum {
                        return Err(StoreError::CrcMismatch { block_index: idx });
                    }
                    let payload: BlockPayload = rmp_serde::from_slice(&block.payload)?;
                    Ok((block.kind, payload))
                })
                .collect::<Result<Vec<_>>>()?
        };

        validate_decoded_blocks(&decoded, &header.string_table)?;

        let mut store = Store::new();
        store.string_table = header.string_table;
        store.next_node_id = 0;
        store.next_edge_id = 0;

        // First pass: nodes.
        for (_, payload) in &decoded {
            if let BlockPayload::Nodes { nodes, .. } = payload {
                for node in nodes {
                    store.next_node_id = store.next_node_id.max(node.id.0 + 1);
                    store
                        .node_label_index
                        .entry(node.label)
                        .or_default()
                        .push(node.id);
                    store.nodes.insert(node.id, node.clone());
                }
            }
        }
        // Second pass: edges.
        for (_, payload) in &decoded {
            if let BlockPayload::Edges { edges, .. } = payload {
                for edge in edges {
                    if !store.nodes.contains_key(&edge.src) || !store.nodes.contains_key(&edge.dst)
                    {
                        return Err(StoreError::DanglingEdge { edge_id: edge.id.0 });
                    }
                    store.next_edge_id = store.next_edge_id.max(edge.id.0 + 1);
                    store
                        .edge_label_index
                        .entry(edge.label)
                        .or_default()
                        .push(edge.id);
                    store.outgoing.entry(edge.src).or_default().push(edge.id);
                    store.incoming.entry(edge.dst).or_default().push(edge.id);
                    store.edges.insert(edge.id, edge.clone());
                }
            }
        }
        // Third pass: reconstruct canonical facts from node/edge provenance.
        let node_facts: Vec<_> = store
            .nodes
            .values()
            .map(|n| (n.id, n.provenance.clone()))
            .collect();
        let edge_facts: Vec<_> = store
            .edges
            .values()
            .map(|e| (e.id, e.provenance.clone()))
            .collect();
        for (id, prov) in node_facts {
            store.add_fact(FactSubject::Node(id), prov);
        }
        for (id, prov) in edge_facts {
            store.add_fact(FactSubject::Edge(id), prov);
        }
        // Fourth pass: append competing facts persisted beyond the canonical one.
        for (_, payload) in &decoded {
            if let BlockPayload::Facts { entries } = payload {
                for (subject, provenance) in entries {
                    match subject {
                        FactSubject::Node(id) if !store.nodes.contains_key(id) => {
                            return Err(StoreError::DanglingFact);
                        }
                        FactSubject::Edge(id) if !store.edges.contains_key(id) => {
                            return Err(StoreError::DanglingFact);
                        }
                        _ => {}
                    }
                    store
                        .facts
                        .entry(*subject)
                        .or_default()
                        .push(provenance.clone());
                }
            }
        }

        Ok(store)
    }
}

fn write_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<()> {
    let bytes = rmp_serde::to_vec(value)?;
    writer.write_all(&(bytes.len() as u64).to_le_bytes())?;
    writer.write_all(&bytes)?;
    Ok(())
}

fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(reader: &mut R) -> Result<T> {
    let mut len_bytes = [0_u8; 8];
    reader.read_exact(&mut len_bytes)?;
    let len = u64::from_le_bytes(len_bytes);
    if len > MAX_FRAME_BYTES {
        return Err(StoreError::FrameTooLarge { len });
    }
    let len = len as usize;
    let mut bytes = vec![0_u8; len];
    reader.read_exact(&mut bytes)?;
    Ok(rmp_serde::from_slice(&bytes)?)
}

fn validate_decoded_blocks(
    decoded: &[(BlockKind, BlockPayload)],
    string_table: &StringTable,
) -> Result<()> {
    for (kind, payload) in decoded {
        match (kind, payload) {
            (BlockKind::Nodes(kind_label), BlockPayload::Nodes { label, nodes })
                if kind_label == label =>
            {
                require_label(string_table, *label)?;
                for node in nodes {
                    if node.label != *label {
                        return Err(StoreError::InconsistentBlockPayload);
                    }
                    require_label(string_table, node.label)?;
                    require_keys(string_table, &node.properties)?;
                }
            }
            (BlockKind::Edges(kind_relation), BlockPayload::Edges { relation, edges })
                if kind_relation == relation =>
            {
                require_relation(string_table, *relation)?;
                for edge in edges {
                    if edge.label != *relation {
                        return Err(StoreError::InconsistentBlockPayload);
                    }
                    require_relation(string_table, edge.label)?;
                    require_keys(string_table, &edge.properties)?;
                }
            }
            (BlockKind::Facts, BlockPayload::Facts { .. }) => {}
            _ => return Err(StoreError::InconsistentBlockPayload),
        }
    }
    Ok(())
}

fn require_label(string_table: &StringTable, label: LabelId) -> Result<()> {
    string_table
        .resolve_label(label)
        .map(|_| ())
        .ok_or(StoreError::UnknownStringId { id: label.0 })
}

fn require_relation(string_table: &StringTable, relation: RelationId) -> Result<()> {
    string_table
        .resolve_relation(relation)
        .map(|_| ())
        .ok_or(StoreError::UnknownStringId { id: relation.0 })
}

fn require_keys(
    string_table: &StringTable,
    properties: &[(crate::id::KeyId, crate::Scalar)],
) -> Result<()> {
    for (key, _) in properties {
        string_table
            .resolve_key(*key)
            .ok_or(StoreError::UnknownStringId { id: key.0 })?;
    }
    Ok(())
}
