//! On-disk storage format: file header, blocks, checksums, and parallel save/load.

use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::id::{LabelId, RelationId};
use crate::node::Node;
use crate::ontology::StringTable;
use crate::provenance::Provenance;
use crate::store::Store;
use ahash::AHashMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use thiserror::Error;

const MAGIC: &[u8; 8] = b"PADAGON\n";
const VERSION: u8 = 1;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("bincode error: {0}")]
    Bincode(#[from] bincode::Error),
    #[error("CRC mismatch in block {block_index}")]
    CrcMismatch { block_index: usize },
    #[error("Bad magic or version")]
    BadHeader,
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
                let bytes = bincode::serialize(&payload)?;
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
                let bytes = bincode::serialize(&payload)?;
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
        let fact_bytes = bincode::serialize(&BlockPayload::Facts {
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
        bincode::serialize_into(&mut writer, &header)?;
        for block in &blocks {
            bincode::serialize_into(&mut writer, block)?;
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
        let header: FileHeader = bincode::deserialize_from(&mut reader)?;
        if &header.magic != MAGIC || header.version != VERSION {
            return Err(StoreError::BadHeader);
        }

        let mut raw_blocks = Vec::with_capacity(header.block_count as usize);
        for _ in 0..header.block_count {
            let block: Block = bincode::deserialize_from(&mut reader)?;
            raw_blocks.push(block);
        }

        let decoded: Vec<(BlockKind, BlockPayload)> = if parallel {
            raw_blocks
                .into_par_iter()
                .enumerate()
                .map(|(idx, block)| {
                    if crc32fast::hash(&block.payload) != block.checksum {
                        return Err(StoreError::CrcMismatch { block_index: idx });
                    }
                    let payload: BlockPayload = bincode::deserialize(&block.payload)?;
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
                    let payload: BlockPayload = bincode::deserialize(&block.payload)?;
                    Ok((block.kind, payload))
                })
                .collect::<Result<Vec<_>>>()?
        };

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
