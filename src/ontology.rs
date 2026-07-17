use crate::id::{KeyId, LabelId, RelationId};
use ahash::AHashMap;
use serde::{Deserialize, Serialize};

/// Interned string table for ontology strings (labels, relations, property keys).
/// Serializable so it can be stored as the global dictionary in a PADAGONIA file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StringTable {
    by_id: Vec<String>,
    by_str: AHashMap<String, u32>,
}

impl StringTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a string and return its compact id. Existing strings return the same id.
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.by_str.get(s) {
            return id;
        }
        let id = self.by_id.len() as u32;
        let owned = s.to_owned();
        self.by_str.insert(owned.clone(), id);
        self.by_id.push(owned);
        id
    }

    /// Resolve an interned id back to its original string.
    pub fn resolve(&self, id: u32) -> Option<&str> {
        self.by_id.get(id as usize).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// Convenience trait for types that can be interned into the ontology.
pub trait Internable {
    fn from_raw(id: u32) -> Self;
}

impl Internable for LabelId {
    fn from_raw(id: u32) -> Self {
        LabelId(id)
    }
}

impl Internable for RelationId {
    fn from_raw(id: u32) -> Self {
        RelationId(id)
    }
}

impl Internable for KeyId {
    fn from_raw(id: u32) -> Self {
        KeyId(id)
    }
}

pub trait StringTableExt {
    fn intern_label(&mut self, s: &str) -> LabelId;
    fn intern_relation(&mut self, s: &str) -> RelationId;
    fn intern_key(&mut self, s: &str) -> KeyId;
    fn label_id(&self, s: &str) -> Option<LabelId>;
    fn relation_id(&self, s: &str) -> Option<RelationId>;
    fn key_id(&self, s: &str) -> Option<KeyId>;
    fn resolve_label(&self, id: LabelId) -> Option<&str>;
    fn resolve_relation(&self, id: RelationId) -> Option<&str>;
    fn resolve_key(&self, id: KeyId) -> Option<&str>;
}

impl StringTableExt for StringTable {
    fn intern_label(&mut self, s: &str) -> LabelId {
        LabelId(self.intern(s))
    }

    fn intern_relation(&mut self, s: &str) -> RelationId {
        RelationId(self.intern(s))
    }

    fn intern_key(&mut self, s: &str) -> KeyId {
        KeyId(self.intern(s))
    }

    fn label_id(&self, s: &str) -> Option<LabelId> {
        self.by_str.get(s).copied().map(LabelId)
    }

    fn relation_id(&self, s: &str) -> Option<RelationId> {
        self.by_str.get(s).copied().map(RelationId)
    }

    fn key_id(&self, s: &str) -> Option<KeyId> {
        self.by_str.get(s).copied().map(KeyId)
    }

    fn resolve_label(&self, id: LabelId) -> Option<&str> {
        self.resolve(id.0)
    }

    fn resolve_relation(&self, id: RelationId) -> Option<&str> {
        self.resolve(id.0)
    }

    fn resolve_key(&self, id: KeyId) -> Option<&str> {
        self.resolve(id.0)
    }
}
