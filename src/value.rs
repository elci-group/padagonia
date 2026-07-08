use serde::{Deserialize, Serialize};

/// A scalar property value. Embeddings are stored separately on Node/Edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Scalar {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
    Embedding(Vec<f32>),
    Timestamp(u64),
}
