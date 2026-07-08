use serde::{Deserialize, Serialize};

/// Provenance metadata attached to every node, edge, and fact.
/// Multiple facts may compete for the same subject; readers can weight by confidence/timestamp.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub agent: String,
    pub model: String,
    pub confidence: f32,
    pub cost: f32,
    pub timestamp: u64,
    pub evidence: Vec<String>,
}

impl Provenance {
    pub fn new(
        agent: impl Into<String>,
        model: impl Into<String>,
        confidence: f32,
        cost: f32,
        timestamp: u64,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            agent: agent.into(),
            model: model.into(),
            confidence,
            cost,
            timestamp,
            evidence,
        }
    }
}
