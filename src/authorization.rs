// Copyright (c) 2026 ViCo Agent
// SPDX-License-Identifier: MIT
//! Tenant authorization and quota primitives.

use crate::identity::NamespaceId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Role {
    Reader,
    Writer,
    Analyst,
    Administrator,
    Billing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Read,
    Write,
    Analyze,
    Administer,
    Bill,
}

impl Role {
    pub fn permits(self, operation: Operation) -> bool {
        match self {
            Self::Reader => matches!(operation, Operation::Read),
            Self::Writer => matches!(operation, Operation::Read | Operation::Write),
            Self::Analyst => matches!(operation, Operation::Read | Operation::Analyze),
            Self::Administrator => true,
            Self::Billing => matches!(operation, Operation::Read | Operation::Bill),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Credential {
    pub token_hash: [u8; 32],
    pub namespace: NamespaceId,
    pub role: Role,
    pub revoked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedPrincipal {
    pub namespace: NamespaceId,
    pub role: Role,
}

#[derive(Clone, Default)]
pub struct CredentialRegistry {
    pub(crate) credentials: HashMap<[u8; 32], Credential>,
}

impl CredentialRegistry {
    pub fn insert(&mut self, credential: Credential) {
        self.credentials.insert(credential.token_hash, credential);
    }

    pub fn insert_token(&mut self, token: &str, namespace: NamespaceId, role: Role) {
        self.insert(Credential {
            token_hash: hash_token(token),
            namespace,
            role,
            revoked: false,
        });
    }

    pub fn revoke(&mut self, token: &str) -> bool {
        self.credentials
            .get_mut(&hash_token(token))
            .is_some_and(|credential| {
                credential.revoked = true;
                true
            })
    }

    pub fn len(&self) -> usize {
        self.credentials.len()
    }

    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }

    pub fn authenticate(
        &self,
        token: &str,
        namespace: &NamespaceId,
    ) -> Option<AuthenticatedPrincipal> {
        let token_hash = hash_token(token);
        self.credentials.values().find_map(|credential| {
            (constant_time_eq(&credential.token_hash, &token_hash)
                && !credential.revoked
                && (&credential.namespace == namespace || credential.role == Role::Administrator))
                .then_some(AuthenticatedPrincipal {
                    namespace: if credential.role == Role::Administrator {
                        namespace.clone()
                    } else {
                        credential.namespace.clone()
                    },
                    role: credential.role,
                })
        })
    }

    pub fn authorize(
        &self,
        token: &str,
        namespace: &NamespaceId,
        operation: Operation,
    ) -> Result<AuthenticatedPrincipal, AuthorizationError> {
        let principal = self
            .authenticate(token, namespace)
            .ok_or(AuthorizationError::Unauthenticated)?;
        if !principal.role.permits(operation) {
            return Err(AuthorizationError::Forbidden);
        }
        Ok(principal)
    }
}

#[derive(Clone, Debug, Default)]
pub struct TenantQuota {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_bytes: u64,
    pub max_requests: u64,
}

#[derive(Clone, Debug, Default)]
pub struct QuotaUsage {
    pub nodes: usize,
    pub edges: usize,
    pub bytes: u64,
    pub requests: u64,
}

#[derive(Default)]
pub struct QuotaRegistry {
    quotas: HashMap<NamespaceId, TenantQuota>,
    usage: HashMap<NamespaceId, QuotaUsage>,
}

impl QuotaRegistry {
    pub fn set_quota(&mut self, namespace: NamespaceId, quota: TenantQuota) {
        self.quotas.insert(namespace, quota);
    }

    pub fn record_request(&mut self, namespace: &NamespaceId) -> Result<(), QuotaError> {
        let usage = self.usage.entry(namespace.clone()).or_default();
        usage.requests = usage.requests.saturating_add(1);
        if self
            .quotas
            .get(namespace)
            .is_some_and(|quota| quota.max_requests > 0 && usage.requests > quota.max_requests)
        {
            usage.requests -= 1;
            return Err(QuotaError::Requests);
        }
        Ok(())
    }

    pub fn reserve_graph(
        &mut self,
        namespace: &NamespaceId,
        nodes: usize,
        edges: usize,
        bytes: u64,
    ) -> Result<(), QuotaError> {
        let quota = self.quotas.get(namespace).cloned().unwrap_or_default();
        let usage = self.usage.entry(namespace.clone()).or_default();
        let exceeds = (quota.max_nodes > 0 && usage.nodes.saturating_add(nodes) > quota.max_nodes)
            || (quota.max_edges > 0 && usage.edges.saturating_add(edges) > quota.max_edges)
            || (quota.max_bytes > 0 && usage.bytes.saturating_add(bytes) > quota.max_bytes);
        if exceeds {
            return Err(QuotaError::Storage);
        }
        usage.nodes += nodes;
        usage.edges += edges;
        usage.bytes += bytes;
        Ok(())
    }

    pub fn usage(&self, namespace: &NamespaceId) -> QuotaUsage {
        self.usage.get(namespace).cloned().unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorizationError {
    Unauthenticated,
    Forbidden,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaError {
    Requests,
    Storage,
}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden => "forbidden",
        })
    }
}
impl fmt::Display for QuotaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Requests => "request quota exceeded",
            Self::Storage => "storage quota exceeded",
        })
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for (left, right) in a.iter().zip(b.iter()) {
        diff |= usize::from(left ^ right);
    }
    diff == 0
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DurableCredential {
    hash_hex: String,
    namespace: NamespaceId,
    role: Role,
    revoked: bool,
}

impl CredentialRegistry {
    fn to_durable(&self) -> Vec<DurableCredential> {
        let mut records: Vec<DurableCredential> = self
            .credentials
            .values()
            .map(|credential| DurableCredential {
                hash_hex: hex_encode(&credential.token_hash),
                namespace: credential.namespace.clone(),
                role: credential.role,
                revoked: credential.revoked,
            })
            .collect();
        records.sort_by(|a, b| a.hash_hex.cmp(&b.hash_hex));
        records
    }

    fn from_durable(records: Vec<DurableCredential>) -> Result<Self, String> {
        let mut credentials = HashMap::new();
        for record in records {
            let hash = hex_decode(&record.hash_hex)
                .ok_or_else(|| format!("invalid credential hash hex: {}", record.hash_hex))?;
            if hash.len() != 32 {
                return Err(format!(
                    "credential hash length must be 32 bytes: {}",
                    record.hash_hex
                ));
            }
            let mut token_hash = [0u8; 32];
            token_hash.copy_from_slice(&hash);
            credentials.insert(
                token_hash,
                Credential {
                    token_hash,
                    namespace: record.namespace,
                    role: record.role,
                    revoked: record.revoked,
                },
            );
        }
        Ok(Self { credentials })
    }

    /// Persist credentials durably with atomic replace and directory fsync.
    pub fn save_to_path(&self, path: &Path) -> Result<(), String> {
        let payload = serde_json::to_vec_pretty(&self.to_durable()).map_err(|e| e.to_string())?;
        write_atomic(&payload, path).map_err(|e| e.to_string())
    }

    /// Load credentials from a durable JSON file.
    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let records: Vec<DurableCredential> =
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        Self::from_durable(records)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(hex_char(b >> 4));
        out.push(hex_char(b & 0x0f));
    }
    out
}

fn hex_char(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!(),
    }
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let chars: Vec<char> = hex.chars().collect();
    for i in (0..hex.len()).step_by(2) {
        let hi = hex_value(chars[i])?;
        let lo = hex_value(chars[i + 1])?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn hex_value(c: char) -> Option<u8> {
    match c {
        '0'..='9' => Some((c as u8) - b'0'),
        'a'..='f' => Some((c as u8) - b'a' + 10),
        'A'..='F' => Some((c as u8) - b'A' + 10),
        _ => None,
    }
}

fn write_atomic(bytes: &[u8], path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let temporary = path.with_extension(format!("credentials.tmp.{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temporary, path)?;
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revocation_and_roles_are_enforced() {
        let namespace = NamespaceId::new("workspace").unwrap();
        let mut registry = CredentialRegistry::default();
        registry.insert_token("secret-token", namespace.clone(), Role::Writer);
        assert!(registry
            .authorize("secret-token", &namespace, Operation::Write)
            .is_ok());
        assert_eq!(
            registry.authorize("secret-token", &namespace, Operation::Bill),
            Err(AuthorizationError::Forbidden)
        );
        registry.revoke("secret-token");
        assert_eq!(
            registry.authorize("secret-token", &namespace, Operation::Read),
            Err(AuthorizationError::Unauthenticated)
        );
    }
    #[test]
    fn quota_reservations_are_atomic() {
        let namespace = NamespaceId::new("workspace").unwrap();
        let mut quotas = QuotaRegistry::default();
        quotas.set_quota(
            namespace.clone(),
            TenantQuota {
                max_nodes: 2,
                max_edges: 1,
                max_bytes: 100,
                max_requests: 1,
            },
        );
        assert!(quotas.reserve_graph(&namespace, 2, 1, 100).is_ok());
        assert_eq!(
            quotas.reserve_graph(&namespace, 1, 0, 0),
            Err(QuotaError::Storage)
        );
        assert_eq!(quotas.usage(&namespace).nodes, 2);
    }
}
