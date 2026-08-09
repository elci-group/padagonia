use crate::id::{KeyId, LabelId, NodeId};
use crate::identity::{default_external_id, default_schema_version, NamespaceId};
use crate::provenance::Provenance;
use crate::value::Scalar;
use serde::{Deserialize, Serialize};

/// An immutable node in the PADAGONIA graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    /// Stable identity used by APIs and idempotent ingestion.
    #[serde(default = "default_external_id")]
    pub external_id: String,
    #[serde(default)]
    pub namespace: NamespaceId,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: Option<u64>,
    #[serde(default = "default_schema_version")]
    pub schema_version: u16,
    pub label: LabelId,
    pub properties: Vec<(KeyId, Scalar)>,
    pub embedding: Option<Vec<f32>>,
    pub provenance: Provenance,
}
