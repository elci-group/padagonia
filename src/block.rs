use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::id::{LabelId, RelationId};
use crate::node::Node;
use crate::ontology::StringTable;
use crate::provenance::Provenance;
use serde::{Deserialize, Serialize};

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
