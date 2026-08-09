mod api;
pub mod auth;
pub mod authorization;
pub mod bench_support;
pub mod benchmark_gate;
pub mod block;
pub mod checksum;
pub mod contract;
pub mod domain;
pub mod edge;
pub mod fact;
pub mod frame;
pub mod hnsw;
pub mod http_error;
pub mod id;
pub mod identity;
pub mod lifecycle;
pub mod metrics;
pub mod migration;
pub mod node;
pub mod ontology;
pub mod outbox;
pub mod projection;
pub mod provenance;
pub mod query;
pub mod store;
pub mod transaction;
pub mod value;

pub mod app_config;
pub mod cli;
pub mod server;
mod server_middleware;
mod server_state;
pub mod storage;

pub use authorization::{
    AuthenticatedPrincipal, AuthorizationError, Credential, CredentialRegistry, Operation,
    QuotaError, QuotaRegistry, QuotaUsage, Role, TenantQuota,
};
pub use benchmark_gate::{
    evaluate as evaluate_benchmark_gate, GateReport, GateThresholds, WorkloadMeasurement,
};
pub use block::{Block, BlockKind, BlockPayload, FileHeader, MAGIC, VERSION};
pub use checksum::{compute_checksum, validate_checksum};
pub use contract::{
    BatchMutationRequest, BatchMutationResponse, NodeQueryRequest, PageCursorResponse, API_VERSION,
};
pub use domain::{NodeKind, RelationKind, REQUIRED_NODE_KINDS, REQUIRED_RELATION_KINDS};
pub use edge::Edge;
pub use fact::FactSubject;
pub use frame::MAX_FRAME_BYTES;
pub use hnsw::{Distance, HnswIndex, HnswParams, PointId};
pub use id::{EdgeId, KeyId, LabelId, NodeId, RelationId};
pub use identity::{stable_external_id, IdentityError, NamespaceId, CURRENT_SCHEMA_VERSION};
pub use lifecycle::{LifecycleError, LifecycleRegistry, RecordKey, RetentionPolicy, Tombstone};
pub use node::Node;
pub use ontology::{StringTable, StringTableExt};
pub use outbox::{
    compare_shadow_reads, Outbox, OutboxError, OutboxEvent, PersistentOutbox, ShadowReadDiff,
};
pub use projection::Projection;
pub use provenance::Provenance;
pub use query::{NodePage, NodeQuery, NumericAggregate, QueryEngine};
pub use storage::{StoreError, MAX_BLOCK_COUNT, MAX_STORE_BYTES};
pub use store::Store;
pub use transaction::{CommitResult, JournalError, Mutation, Transaction, TransactionJournal};
pub use value::Scalar;
