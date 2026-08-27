// Copyright (c) 2026 ViCo Agent
// SPDX-License-Identifier: MIT
//! Versioned software decomposition contract for transformation consumers.
//!
//! Padagonia establishes component boundaries and relationships. It does not
//! attach licensing acceptability or transformation recommendations to them.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SOFTWARE_ONTOLOGY_SCHEMA_VERSION: &str = "padagonia.software-ontology/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoftwareComponentKind {
    Application,
    Repository,
    Package,
    Module,
    Subsystem,
    Component,
    Interface,
    Implementation,
    GeneratedArtifact,
    Adapter,
    Integration,
    Utility,
    Configuration,
    Test,
    BuildTimeComponent,
    RuntimeComponent,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoftwareRelationshipKind {
    Contains,
    DependsOn,
    Implements,
    Wraps,
    DerivesFrom,
    Calls,
    Generates,
    Adapts,
    Embeds,
    Duplicates,
    InterfacesWith,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransformationBoundary {
    pub independently_addressable: bool,
    pub independently_transformable: bool,
    pub boundary_established: bool,
    pub granularity: u32,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoftwareComponent {
    pub id: String,
    pub name: String,
    pub kind: SoftwareComponentKind,
    pub path: Option<String>,
    pub boundary: TransformationBoundary,
    pub attributes: BTreeMap<String, String>,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoftwareRelationship {
    pub source: String,
    pub target: String,
    pub kind: SoftwareRelationshipKind,
    pub confidence: f64,
    pub evidence_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RelationshipSet {
    pub relationships: Vec<SoftwareRelationship>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecompositionConfidence {
    pub overall: f64,
    pub component_boundaries: BTreeMap<String, f64>,
    pub unresolved_boundaries: Vec<String>,
}

impl DecompositionConfidence {
    #[must_use]
    pub fn for_component(&self, component: &str) -> f64 {
        self.component_boundaries
            .get(component)
            .copied()
            .unwrap_or(self.overall)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComponentGraph {
    pub root: String,
    pub components: Vec<SoftwareComponent>,
    pub relationships: RelationshipSet,
    pub decomposition_confidence: DecompositionConfidence,
}

impl ComponentGraph {
    pub fn validate(&self) -> Result<(), String> {
        let ids: BTreeSet<&str> = self.components.iter().map(|item| item.id.as_str()).collect();
        if !ids.contains(self.root.as_str()) {
            return Err(format!("root component '{}' is missing", self.root));
        }
        if ids.len() != self.components.len() {
            return Err("component identifiers must be unique".to_string());
        }
        for relationship in &self.relationships.relationships {
            if !ids.contains(relationship.source.as_str())
                || !ids.contains(relationship.target.as_str())
            {
                return Err("relationship endpoint is not present in the component graph".to_string());
            }
            if !valid_confidence(relationship.confidence) {
                return Err("relationship confidence must be between zero and one".to_string());
            }
        }
        if !valid_confidence(self.decomposition_confidence.overall)
            || self
                .decomposition_confidence
                .component_boundaries
                .values()
                .any(|confidence| !valid_confidence(*confidence))
        {
            return Err("decomposition confidence must be between zero and one".to_string());
        }
        Ok(())
    }

    #[must_use]
    pub fn component(&self, id: &str) -> Option<&SoftwareComponent> {
        self.components.iter().find(|component| component.id == id)
    }

    #[must_use]
    pub fn children(&self, id: &str) -> Vec<&SoftwareComponent> {
        self.relationships
            .relationships
            .iter()
            .filter(|relationship| {
                relationship.source == id && relationship.kind == SoftwareRelationshipKind::Contains
            })
            .filter_map(|relationship| self.component(&relationship.target))
            .collect()
    }

    #[must_use]
    pub fn descendants(&self, id: &str) -> Vec<&SoftwareComponent> {
        let mut found = Vec::new();
        let mut visited = BTreeSet::new();
        let mut pending = vec![id];
        while let Some(parent) = pending.pop() {
            for child in self.children(parent) {
                if visited.insert(child.id.as_str()) {
                    pending.push(child.id.as_str());
                    found.push(child);
                }
            }
        }
        found
    }

    #[must_use]
    pub fn can_descend(&self, id: &str, minimum_confidence: f64, minimum_granularity: u32) -> bool {
        self.component(id).is_some_and(|component| {
            component.boundary.boundary_established
                && component.boundary.granularity > minimum_granularity
                && self.decomposition_confidence.for_component(id) >= minimum_confidence
                && !self.children(id).is_empty()
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ontology {
    pub schema_version: String,
    pub namespace: String,
    pub graph: ComponentGraph,
    pub vocabulary: BTreeMap<String, String>,
}

impl Ontology {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SOFTWARE_ONTOLOGY_SCHEMA_VERSION {
            return Err(format!("unsupported ontology schema '{}'", self.schema_version));
        }
        self.graph.validate()
    }
}

const fn valid_confidence(value: f64) -> bool {
    value.is_finite() && value >= 0.0 && value <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(id: &str, granularity: u32) -> SoftwareComponent {
        SoftwareComponent {
            id: id.to_string(),
            name: id.to_string(),
            kind: SoftwareComponentKind::Component,
            path: None,
            boundary: TransformationBoundary {
                independently_addressable: true,
                independently_transformable: true,
                boundary_established: true,
                granularity,
                reasons: Vec::new(),
            },
            attributes: BTreeMap::new(),
            evidence_ids: Vec::new(),
        }
    }

    #[test]
    fn traverses_containment_without_following_other_relationships() {
        let graph = ComponentGraph {
            root: "x".to_string(),
            components: vec![component("x", 2), component("x/f", 1), component("external", 1)],
            relationships: RelationshipSet {
                relationships: vec![
                    SoftwareRelationship {
                        source: "x".to_string(),
                        target: "x/f".to_string(),
                        kind: SoftwareRelationshipKind::Contains,
                        confidence: 0.95,
                        evidence_ids: Vec::new(),
                    },
                    SoftwareRelationship {
                        source: "x/f".to_string(),
                        target: "external".to_string(),
                        kind: SoftwareRelationshipKind::DependsOn,
                        confidence: 0.9,
                        evidence_ids: Vec::new(),
                    },
                ],
            },
            decomposition_confidence: DecompositionConfidence {
                overall: 0.9,
                component_boundaries: BTreeMap::new(),
                unresolved_boundaries: Vec::new(),
            },
        };
        assert!(graph.validate().is_ok());
        assert_eq!(graph.descendants("x").len(), 1);
        assert!(graph.can_descend("x", 0.8, 0));
    }

    #[test]
    fn serializes_versioned_ontology() {
        let ontology = Ontology {
            schema_version: SOFTWARE_ONTOLOGY_SCHEMA_VERSION.to_string(),
            namespace: "example".to_string(),
            graph: ComponentGraph {
                root: "x".to_string(),
                components: vec![component("x", 1)],
                relationships: RelationshipSet::default(),
                decomposition_confidence: DecompositionConfidence {
                    overall: 1.0,
                    component_boundaries: BTreeMap::new(),
                    unresolved_boundaries: Vec::new(),
                },
            },
            vocabulary: BTreeMap::new(),
        };
        assert!(ontology.validate().is_ok());
        let json = serde_json::to_value(ontology).expect("ontology should serialize");
        assert_eq!(json["schema_version"], SOFTWARE_ONTOLOGY_SCHEMA_VERSION);
    }
}
