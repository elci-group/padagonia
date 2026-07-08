use crate::id::{KeyId, LabelId, NodeId};
use crate::provenance::Provenance;
use crate::value::Scalar;
use serde::{Deserialize, Serialize};

/// An immutable node in the PADAGONIA graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub label: LabelId,
    pub properties: Vec<(KeyId, Scalar)>,
    pub embedding: Option<Vec<f32>>,
    pub provenance: Provenance,
}
