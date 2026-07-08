use crate::id::{EdgeId, KeyId, NodeId, RelationId};
use crate::provenance::Provenance;
use crate::value::Scalar;
use serde::{Deserialize, Serialize};

/// An immutable directed edge in the PADAGONIA graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub src: NodeId,
    pub dst: NodeId,
    pub label: RelationId,
    pub properties: Vec<(KeyId, Scalar)>,
    pub embedding: Option<Vec<f32>>,
    pub provenance: Provenance,
}
