//! Versioned wire contract models. Internal maps and storage files stay private.

use crate::identity::NamespaceId;
use crate::query::NodeQuery;
use crate::transaction::{CommitResult, Mutation, Transaction};
use serde::{Deserialize, Serialize};

pub const API_VERSION: &str = "v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchMutationRequest {
    pub namespace: NamespaceId,
    pub idempotency_key: String,
    pub mutations: Vec<Mutation>,
}
impl BatchMutationRequest {
    pub fn into_transaction(self) -> Transaction {
        Transaction {
            idempotency_key: self.idempotency_key,
            mutations: self.mutations,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchMutationResponse {
    pub api_version: String,
    pub result: CommitResult,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeQueryRequest {
    pub namespace: NamespaceId,
    pub label: Option<u32>,
    pub created_after: Option<u64>,
    pub created_before: Option<u64>,
    pub limit: usize,
    pub cursor: Option<String>,
}
impl NodeQueryRequest {
    pub fn into_query(self) -> NodeQuery {
        NodeQuery {
            namespace: Some(self.namespace),
            label: self.label.map(crate::LabelId),
            created_after: self.created_after,
            created_before: self.created_before,
            limit: self.limit,
            cursor: self.cursor,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PageCursorResponse {
    pub api_version: String,
    pub node_ids: Vec<u64>,
    pub next_cursor: Option<String>,
}
