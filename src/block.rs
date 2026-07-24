//! Block encoding/decoding and data structures for storage.

use crate::checksum::compute_checksum;
use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::id::{LabelId, RelationId};
use crate::node::Node;
use crate::provenance::Provenance;
use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8; 8] = b"PADAGON\n";
pub const VERSION: u8 = 2;

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
    pub string_table: crate::ontology::StringTable,
    pub block_count: u64,
}

impl Block {
    pub fn new(kind: BlockKind, payload: Vec<u8>) -> Self {
        let checksum = compute_checksum(&payload);
        Self {
            kind,
            payload,
            checksum,
        }
    }
}

impl FileHeader {
    pub fn new(string_table: crate::ontology::StringTable, block_count: u64) -> Self {
        Self {
            magic: *MAGIC,
            version: VERSION,
            string_table,
            block_count,
        }
    }
}
