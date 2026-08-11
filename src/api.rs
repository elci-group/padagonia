//! HTTP data-transfer models and boundary validation.

use crate::app_config::LimitsConfig;
use crate::http_error::{bad_request, ApiResult};
use crate::provenance::Provenance;
use crate::store::Store;
use crate::value::Scalar;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
pub(crate) struct StatsResponse {
    pub(crate) nodes: usize,
    pub(crate) edges: usize,
    pub(crate) facts: usize,
    pub(crate) labels: usize,
    pub(crate) relations: usize,
}

#[derive(Serialize)]
pub(crate) struct IdResponse {
    pub(crate) id: u64,
}

#[derive(Deserialize)]
pub(crate) struct IngestRequest {
    pub(crate) nodes: usize,
    pub(crate) edges: usize,
    pub(crate) seed: u64,
}

#[derive(Deserialize)]
pub(crate) struct ProvenanceInput {
    agent: String,
    model: String,
    confidence: Option<f32>,
    cost: Option<f32>,
    evidence: Option<Vec<String>>,
}

impl ProvenanceInput {
    pub(crate) fn into_provenance(self) -> Provenance {
        Provenance {
            agent: self.agent,
            model: self.model,
            confidence: self.confidence.unwrap_or(1.0),
            cost: self.cost.unwrap_or(0.0),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs()),
            evidence: self.evidence.unwrap_or_default(),
        }
    }
}

pub(crate) fn default_provenance() -> Provenance {
    Provenance {
        agent: "http-api".to_string(),
        model: "unknown".to_string(),
        confidence: 1.0,
        cost: 0.0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs()),
        evidence: Vec::new(),
    }
}

fn json_to_scalar(value: serde_json::Value) -> Scalar {
    match value {
        serde_json::Value::Null => Scalar::Null,
        serde_json::Value::Bool(value) => Scalar::Bool(value),
        serde_json::Value::Number(number) => number
            .as_i64()
            .map_or_else(|| Scalar::F64(number.as_f64().unwrap_or(0.0)), Scalar::I64),
        serde_json::Value::String(value) => Scalar::String(value),
        other => Scalar::String(other.to_string()),
    }
}

pub(crate) fn json_props(properties: &HashMap<String, serde_json::Value>) -> Vec<(&str, Scalar)> {
    properties
        .iter()
        .map(|(key, value)| (key.as_str(), json_to_scalar(value.clone())))
        .collect()
}

pub(crate) fn validate_label(label: &str) -> ApiResult<()> {
    if label.trim().is_empty() {
        return Err(bad_request("label must not be empty"));
    }
    if label.len() > 1_024 {
        return Err(bad_request("label exceeds 1024 UTF-8 bytes"));
    }
    Ok(())
}

pub(crate) fn validate_properties(
    properties: &HashMap<String, serde_json::Value>,
) -> ApiResult<()> {
    if properties.len() > 1_024 {
        return Err(bad_request("property count exceeds 1024"));
    }
    if properties.keys().any(|key| key.trim().is_empty()) {
        return Err(bad_request("property keys must not be empty"));
    }
    Ok(())
}

pub(crate) fn validate_provenance(provenance: Option<&ProvenanceInput>) -> ApiResult<()> {
    let Some(provenance) = provenance else {
        return Ok(());
    };
    if provenance.agent.trim().is_empty() || provenance.model.trim().is_empty() {
        return Err(bad_request("provenance agent and model must not be empty"));
    }
    if provenance
        .confidence
        .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        return Err(bad_request("confidence must be finite and between 0 and 1"));
    }
    if provenance
        .cost
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(bad_request("cost must be finite and non-negative"));
    }
    if provenance
        .evidence
        .as_ref()
        .is_some_and(|items| items.len() > 1_024)
    {
        return Err(bad_request("evidence item count exceeds 1024"));
    }
    Ok(())
}

pub(crate) fn validate_embedding(
    embedding: Option<&[f32]>,
    limits: &LimitsConfig,
) -> ApiResult<()> {
    let Some(embedding) = embedding else {
        return Ok(());
    };
    if embedding.is_empty() {
        return Err(bad_request("embedding must not be empty"));
    }
    if embedding.len() > limits.max_vector_dimensions {
        return Err(bad_request(format!(
            "embedding dimension exceeds {}",
            limits.max_vector_dimensions
        )));
    }
    if embedding.iter().any(|value| !value.is_finite()) {
        return Err(bad_request("embedding values must be finite"));
    }
    Ok(())
}

pub(crate) fn expected_embedding_dimension(store: &Store) -> Option<usize> {
    store
        .nodes()
        .values()
        .find_map(|node| node.embedding.as_ref().map(Vec::len))
}

#[derive(Deserialize)]
pub(crate) struct CreateNodeRequest {
    #[serde(default)]
    pub(crate) namespace: crate::identity::NamespaceId,
    pub(crate) external_id: Option<String>,
    pub(crate) idempotency_key: Option<String>,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) properties: HashMap<String, serde_json::Value>,
    pub(crate) embedding: Option<Vec<f32>>,
    pub(crate) provenance: Option<ProvenanceInput>,
}

#[derive(Deserialize)]
pub(crate) struct CreateEdgeRequest {
    #[serde(default)]
    pub(crate) namespace: crate::identity::NamespaceId,
    pub(crate) external_id: Option<String>,
    pub(crate) idempotency_key: Option<String>,
    pub(crate) src: u64,
    pub(crate) dst: u64,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) properties: HashMap<String, serde_json::Value>,
    pub(crate) embedding: Option<Vec<f32>>,
    pub(crate) provenance: Option<ProvenanceInput>,
}

#[derive(Deserialize)]
pub(crate) struct BfsRequest {
    pub(crate) start: u64,
    pub(crate) depth: usize,
    pub(crate) relation: Option<String>,
    pub(crate) min_confidence: Option<f32>,
}

#[derive(Serialize)]
pub(crate) struct BfsEntry {
    pub(crate) node_id: u64,
    pub(crate) depth: usize,
}

#[derive(Deserialize)]
pub(crate) struct VectorSearchRequest {
    pub(crate) query: Vec<f32>,
    pub(crate) k: Option<usize>,
    pub(crate) ef: Option<usize>,
    pub(crate) label: Option<String>,
    pub(crate) metric: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct VectorHit {
    pub(crate) node_id: u64,
    pub(crate) distance: f32,
}

#[derive(Deserialize)]
pub(crate) struct TombstoneRequest {
    pub(crate) namespace: crate::identity::NamespaceId,
    pub(crate) external_id: String,
    pub(crate) reason: String,
    pub(crate) schema_version: u16,
}

#[derive(Serialize)]
pub(crate) struct NodeResponse {
    pub(crate) id: u64,
    pub(crate) label: String,
    pub(crate) properties: serde_json::Value,
    pub(crate) embedding: Option<Vec<f32>>,
    pub(crate) provenance: Provenance,
}
