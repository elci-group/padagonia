//! Retention, correction, and deletion primitives for tenant data.

use crate::identity::NamespaceId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct RecordKey {
    pub namespace: NamespaceId,
    pub external_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tombstone {
    pub key: RecordKey,
    pub deleted_at: u64,
    pub reason: String,
    pub schema_version: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetentionPolicy {
    pub retention_seconds: u64,
    pub preserve_audit: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    EmptyId,
    Tombstoned,
    SelfSupersession,
}

#[derive(Clone, Debug, Default)]
pub struct LifecycleRegistry {
    tombstones: HashMap<RecordKey, Tombstone>,
    retractions: HashSet<RecordKey>,
    superseded_by: HashMap<RecordKey, RecordKey>,
}

impl LifecycleRegistry {
    pub fn retract(&mut self, key: RecordKey) -> Result<bool, LifecycleError> {
        if key.external_id.is_empty() {
            return Err(LifecycleError::EmptyId);
        }
        Ok(self.retractions.insert(key))
    }

    pub fn supersede(
        &mut self,
        old: RecordKey,
        replacement: RecordKey,
    ) -> Result<(), LifecycleError> {
        if old.external_id.is_empty() || replacement.external_id.is_empty() {
            return Err(LifecycleError::EmptyId);
        }
        if old == replacement {
            return Err(LifecycleError::SelfSupersession);
        }
        self.superseded_by.insert(old, replacement);
        Ok(())
    }

    pub fn tombstone(&mut self, tombstone: Tombstone) -> Result<(), LifecycleError> {
        if tombstone.key.external_id.is_empty() {
            return Err(LifecycleError::EmptyId);
        }
        self.tombstones.insert(tombstone.key.clone(), tombstone);
        Ok(())
    }

    pub fn accepts_replay(&self, key: &RecordKey) -> Result<(), LifecycleError> {
        if self.tombstones.contains_key(key) {
            Err(LifecycleError::Tombstoned)
        } else {
            Ok(())
        }
    }

    pub fn is_retracted(&self, key: &RecordKey) -> bool {
        self.retractions.contains(key)
    }
    pub fn replacement(&self, key: &RecordKey) -> Option<&RecordKey> {
        self.superseded_by.get(key)
    }
    pub fn get_tombstone(&self, key: &RecordKey) -> Option<&Tombstone> {
        self.tombstones.get(key)
    }

    pub fn deletion_plan(
        &self,
        namespace: &NamespaceId,
        keys: impl IntoIterator<Item = RecordKey>,
    ) -> Vec<RecordKey> {
        keys.into_iter()
            .filter(|key| &key.namespace == namespace && !self.tombstones.contains_key(key))
            .collect()
    }

    pub fn eligible_for_compaction(
        &self,
        key: &RecordKey,
        now: u64,
        policy: &RetentionPolicy,
    ) -> bool {
        self.tombstones.get(key).is_some_and(|tombstone| {
            now.saturating_sub(tombstone.deleted_at) >= policy.retention_seconds
        })
    }

    pub fn tombstones(&self) -> impl Iterator<Item = &Tombstone> {
        self.tombstones.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(namespace: &str, id: &str) -> RecordKey {
        RecordKey {
            namespace: NamespaceId::new(namespace).unwrap(),
            external_id: id.into(),
        }
    }
    #[test]
    fn stale_replay_is_blocked_and_retention_is_bounded() {
        let record = key("workspace", "run-1");
        let mut registry = LifecycleRegistry::default();
        registry
            .tombstone(Tombstone {
                key: record.clone(),
                deleted_at: 10,
                reason: "user deletion".into(),
                schema_version: 1,
            })
            .unwrap();
        assert_eq!(
            registry.accepts_replay(&record),
            Err(LifecycleError::Tombstoned)
        );
        assert!(!registry.eligible_for_compaction(
            &record,
            19,
            &RetentionPolicy {
                retention_seconds: 10,
                preserve_audit: true
            }
        ));
        assert!(registry.eligible_for_compaction(
            &record,
            20,
            &RetentionPolicy {
                retention_seconds: 10,
                preserve_audit: true
            }
        ));
    }
    #[test]
    fn supersession_and_namespace_plan_are_scoped() {
        let old = key("workspace", "old");
        let new = key("workspace", "new");
        let other = key("other", "record");
        let mut registry = LifecycleRegistry::default();
        registry.supersede(old.clone(), new.clone()).unwrap();
        assert_eq!(registry.replacement(&old), Some(&new));
        assert_eq!(
            registry
                .deletion_plan(&old.namespace.clone(), vec![old, new, other])
                .len(),
            2
        );
    }
}
