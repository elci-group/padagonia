//! Tenant authorization and quota primitives.

use crate::identity::NamespaceId;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
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
    pub token: String,
    pub namespace: NamespaceId,
    pub role: Role,
    pub revoked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedPrincipal {
    pub namespace: NamespaceId,
    pub role: Role,
}

#[derive(Default)]
pub struct CredentialRegistry {
    credentials: HashMap<String, Credential>,
}

impl CredentialRegistry {
    pub fn insert(&mut self, credential: Credential) {
        self.credentials
            .insert(credential.token.clone(), credential);
    }

    pub fn revoke(&mut self, token: &str) -> bool {
        self.credentials.get_mut(token).is_some_and(|credential| {
            credential.revoked = true;
            true
        })
    }

    pub fn authenticate(
        &self,
        token: &str,
        namespace: &NamespaceId,
    ) -> Option<AuthenticatedPrincipal> {
        self.credentials.values().find_map(|credential| {
            (constant_time_eq(credential.token.as_bytes(), token.as_bytes())
                && !credential.revoked
                && &credential.namespace == namespace)
                .then_some(AuthenticatedPrincipal {
                    namespace: credential.namespace.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revocation_and_roles_are_enforced() {
        let namespace = NamespaceId::new("workspace").unwrap();
        let mut registry = CredentialRegistry::default();
        registry.insert(Credential {
            token: "secret-token".into(),
            namespace: namespace.clone(),
            role: Role::Writer,
            revoked: false,
        });
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
