//! Stable, tenant-scoped identities for persisted graph records.

use serde::{Deserialize, Serialize};
use std::fmt;

pub const CURRENT_SCHEMA_VERSION: u16 = 1;

/// A validated account/workspace namespace.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NamespaceId(String);

impl NamespaceId {
    pub const DEFAULT: &'static str = "default";

    pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        if value.is_empty() || value.len() > 256 {
            return Err(IdentityError::InvalidNamespace);
        }
        if value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
        }) {
            return Err(IdentityError::InvalidNamespace);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for NamespaceId {
    fn default() -> Self {
        Self(Self::DEFAULT.to_owned())
    }
}

impl fmt::Display for NamespaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityError {
    InvalidNamespace,
    EmptyExternalId,
    NamespaceMismatch,
    DuplicateExternalId,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNamespace => {
                f.write_str("namespace is empty, too long, or contains invalid characters")
            }
            Self::EmptyExternalId => f.write_str("external id must not be empty"),
            Self::NamespaceMismatch => {
                f.write_str("record endpoints must share the requested namespace")
            }
            Self::DuplicateExternalId => f.write_str("external id already exists in the namespace"),
        }
    }
}

impl std::error::Error for IdentityError {}

/// Deterministically derives an externally safe identity from canonical input.
/// This is intentionally dependency-free and stable across processes.
pub fn stable_external_id(namespace: &NamespaceId, kind: &str, canonical: &str) -> String {
    let mut first = 0xcbf29ce484222325_u64;
    let mut second = 0x84222325cbf29ce4_u64;
    for byte in namespace
        .as_str()
        .bytes()
        .chain([0])
        .chain(kind.bytes())
        .chain([0])
        .chain(canonical.bytes())
    {
        first = (first ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        second = (second ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        second ^= first.rotate_left(17);
    }
    format!("{kind}_{first:016x}{second:016x}")
}

pub fn now_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub fn default_schema_version() -> u16 {
    CURRENT_SCHEMA_VERSION
}

pub fn default_external_id() -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ids_are_repeatable_and_namespace_scoped() {
        let alpha = NamespaceId::new("account-a").unwrap();
        let beta = NamespaceId::new("account-b").unwrap();
        assert_eq!(
            stable_external_id(&alpha, "Run", "source=one;sequence=1"),
            stable_external_id(&alpha, "Run", "source=one;sequence=1")
        );
        assert_ne!(
            stable_external_id(&alpha, "Run", "source=one;sequence=1"),
            stable_external_id(&beta, "Run", "source=one;sequence=1")
        );
    }

    #[test]
    fn namespace_rejects_unsafe_values() {
        assert!(NamespaceId::new("account/secret").is_err());
        assert!(NamespaceId::new("account.good:v1").is_ok());
    }
}
