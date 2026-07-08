use crate::id::{EdgeId, NodeId};
use serde::{Deserialize, Serialize};

/// The subject a fact is about.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum FactSubject {
    Node(NodeId),
    Edge(EdgeId),
}
