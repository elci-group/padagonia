# PADAGONIA SQL-Replacement Directive

Status: implementation directive v3 — executable closure contract; SQL cutover remains blocked until the final readiness report is green  
Scope: PADAGONIA server and Dreamsequence platform persistence  
Decision owner: PADAGONIA maintainers  

### Closure rule

Each gap is closed only by a production code path, a durable on-disk format or
recovery rule, an HTTP contract, an adversarial test, and a recorded benchmark
measurement. “Module exists” and “unit test passes” are insufficient. The
implementation sequence below therefore treats the following as release
artifacts: a migration manifest, a recovery/restore transcript, an
authorization matrix, a lifecycle retention report, an API contract report,
and a machine-readable SQL comparison result. Any missing artifact is a hard
no-go, regardless of performance.

### Non-negotiable design decisions

- PADAGONIA is single-writer per graph, with journal commit, snapshot replace,
  and journal checkpoint serialized by one commit gate.
- A snapshot is never treated as a journal checkpoint by its size or emptiness;
  replay is identity-aware and applies only absent external IDs.
- Journal compaction retains durable idempotency receipts and the last commit
  sequence; compaction may not erase replay protection.
- Credentials are scoped to a namespace and role, stored as hashes, revocable
  without restart, and audited. The bootstrap administrator key is a
  temporary compatibility mechanism, not the tenant authorization model.
- All mutation routes compile to the same transaction path. Synthetic ingest
  is a benchmark/admin adapter and cannot bypass the journal.
- Lifecycle state is part of the durable snapshot and journal, not an
  in-memory side registry. Tombstones are checked before replay and before
  external-ID reuse.
- The benchmark harness fails closed on data loss, tenant leakage, nonzero
  error rates, or incomparable workloads; a faster but unsafe result is not a
  pass.

## 1. Directive

PADAGONIA shall evolve from a compact graph store into the authoritative
server-side persistence layer for Dreamsequence. SQL may remain as an external
ingestion source, including Codex's own `logs_2.sqlite`, during migration, but
no Dreamsequence server state may depend on SQL after cutover.

PADAGONIA must earn the claim of superiority through reproducible evidence. The
target is not feature parity with SQL syntax. The target is a safer and more
useful substrate for an engineering-intelligence platform whose primary
objects are connected claims, evidence, provenance, capabilities, and
workflows.

The project must not remove SQL merely because graph storage is more attractive
for relationship queries. SQL replacement is complete only when PADAGONIA
provides equivalent or better guarantees for every production workload that
currently relies on SQL.

This directive is also a wiring contract. A capability implemented only as an
unused module, unit-test helper, or documentation claim is not considered
closed. Every required property must be reachable through the server API,
persisted by the production recovery path, covered by an integration test, and
represented in the readiness report.

## 1.1 Current revision gap register

The following blockers must be closed before the replacement claim can be
considered:

- all mutating HTTP routes must use the transaction journal; legacy direct
  mutation routes must become journal-backed compatibility adapters;
- recovery must replay commits after the snapshot checkpoint, not only when the
  snapshot happens to be empty;
- journal checkpointing/compaction must bound replay time and disk growth;
- the server must use namespace-scoped, role-aware, revocable credentials
  instead of a single global API key;
- quotas and lifecycle/tombstone state must be durable and enforced by the
  server, not merely available as in-memory helper types;
- query pagination, aggregates, exact lookup, and tenant filters must be
  exposed through the versioned HTTP contract;
- schema migrations must provide at least one tested forward migration path or
  explicitly prove a clean rebuild from the journal;
- the benchmark contract must execute comparable PADAGONIA/SQLite workloads,
  capture raw results, and fail closed on safety violations;
- production constructors and recovery paths must return structured errors
  rather than panic on journal/storage initialization;
- the operational documentation must state the remaining single-node limits,
  backup procedure, restore procedure, and absence of replication until those
  features are implemented.

## 2. Scope and non-scope

The replacement boundary covers the Dreamsequence control plane:

- workspaces, accounts, memberships, and roles;
- device pairing, token hashes, revocation, and audit events;
- runs, sources, repositories, opportunities, capabilities, and evidence;
- inference accounting, idempotency records, webhook receipts, and retention;
- subscriptions and billing references, but never card data or provider secrets.

The boundary does not require PADAGONIA to replace a database owned by another
application. Dreamseq may continue reading Codex SQLite through a compatibility
adapter, or switch to Codex JSONL/event sources when available. That adapter is
an ingestion concern, not a server-side Dreamsequence database dependency.

## 3. Required graph model

All persistent objects must have a stable external identifier, tenant scope,
creation time, update time where applicable, schema version, and provenance.
Identifiers must be deterministic for idempotent ingestion; random internal
node IDs must not be exposed as the only public identity.

Required node kinds:

| Kind | Purpose |
| --- | --- |
| `Account` | Billing and identity boundary. Do not store provider secrets. |
| `Workspace` | Tenant and policy boundary. |
| `Principal` | Clerk/user/service identity reference. |
| `Device` | Paired installation with hashed, revocable credentials. |
| `Run` | One Dreamseq analysis result and its pipeline counters. |
| `Source` | Agent, repository, CI, terminal, or other telemetry origin. |
| `Repository` | A repository or project observed by a source. |
| `Pattern` | Repeated behavior or engineering friction. |
| `Opportunity` | Evidence-backed capability gap. |
| `Capability` | Existing, extended, proposed, validated, or shipped capability. |
| `InferenceRequest` | Bounded provider usage and outcome accounting. |
| `WebhookReceipt` | Provider event idempotency record. |
| `AuditEvent` | Security-relevant action with safe context. |
| `Subscription` | Non-secret billing state and provider references. |

Required relationship kinds include `contains`, `member_of`, `paired_to`,
`emitted`, `observed_in`, `belongs_to`, `repeats`, `supports`, `suggests`,
`extends`, `implements`, `validated_by`, `released_as`, `charged_to`, and
`supersedes`.

Raw telemetry, bearer tokens, provider API keys, card data, and unredacted
conversation bodies must not be stored in the graph by default. Evidence must
be represented as bounded, redacted references with retention metadata.

## 4. Capabilities required before SQL removal

### 4.1 Transactions and durability

PADAGONIA must provide a durable mutation transaction with:

- atomic batches across nodes, edges, indexes, and secondary projections;
- read-your-write behavior within a request;
- idempotency keys with replay-safe results;
- a write-ahead journal or equivalent crash-recovery protocol;
- commit records that can be replayed deterministically;
- bounded recovery time and a documented recovery point objective;
- snapshot, restore, checksum verification, and backup shipping;
- no whole-database rewrite for ordinary single-object mutations.

The current whole-store clone and replacement persistence path is acceptable for
prototype snapshots but is not sufficient for a multi-tenant control plane.

### 4.2 Tenant isolation and authorization

Every read and write must carry an authenticated tenant context. PADAGONIA must
support:

- workspace/account namespaces with enforced isolation at the storage layer;
- reader, writer, analyst, administrator, and billing roles;
- scoped API keys and revocation without process restart;
- per-tenant quotas for nodes, edges, bytes, vectors, requests, and query cost;
- audit records for accepted and rejected authorization decisions;
- constant-time credential comparison and secret-free diagnostics.

An application-level `workspace_id` filter is insufficient. The graph engine
must reject cross-namespace references before mutation or response generation.

### 4.3 Lifecycle, correction, and privacy

Append-only provenance is useful, but SQL replacement cannot ignore correction
or deletion duties. Implement:

- logical retractions and supersession edges;
- retention policies by node kind and tenant;
- namespace-scoped deletion jobs;
- tombstones that prevent stale replay from resurrecting deleted data;
- compaction that preserves the declared audit and provenance contract;
- evidence redaction and secret scrubbing before persistence;
- export and deletion reports suitable for support and compliance workflows.

The engine must document which data survives snapshots and backups.

### 4.4 Indexes and query execution

Provide typed, bounded query primitives rather than exposing unrestricted graph
traversal:

- exact lookup by external ID and tenant;
- unique constraints for idempotency, device identity, webhook receipt, and
  `(tenant, run, opportunity)` keys;
- indexed equality/range filters over timestamps, status, priority, confidence,
  and retention state;
- ordered pagination with stable cursors;
- bounded BFS and relationship expansion;
- grouped counts, sums, min/max, and time-window aggregates;
- vector search with tenant and label filters;
- query plans with explicit cost and result limits;
- structured explain/diagnostic output without graph contents.

Graph traversal and HNSW similarity are the differentiators. They must be
combined with predictable operational query primitives rather than assuming
that every dashboard query is a traversal.

### 4.5 API and client contract

Publish a versioned API for:

- transactional batch mutations;
- idempotent ingestion;
- namespace-scoped queries;
- audit and retention operations;
- snapshots and restore verification;
- health, readiness, metrics, and query-cost telemetry.

Generate and contract-test Rust, TypeScript, PHP, and Python clients. The
Dreamsequence API must not access PADAGONIA's internal Rust maps or storage
files directly.

## 5. Dreamsequence migration plan

### Phase A — compatibility foundation

1. Add stable external IDs and schema-versioned graph records.
2. Add namespace-aware batch mutation and idempotency.
3. Add the missing query primitives and contract tests.
4. Keep the existing SQL API as the canonical writer.
5. Project committed SQL records to PADAGONIA through an outbox.

### Phase B — derived intelligence cutover

1. Move opportunity discovery, pattern similarity, and capability lineage to
   PADAGONIA queries.
2. Rebuild the graph from the outbox and compare results with SQL read models.
3. Run shadow reads for at least one complete retention period.
4. Measure correctness, latency, memory, storage, recovery, and tenant leakage.
5. Make PADAGONIA the canonical reader for analytics and engineering views.

### Phase C — control-plane cutover

1. Write new runs, devices, audit events, inference receipts, and webhook
   receipts directly to PADAGONIA transactions.
2. Preserve the old SQL database as a read-only rollback artifact.
3. Run dual verification for pairing, revocation, billing webhooks, and
   idempotent ingestion.
4. Migrate billing references and subscription state; keep Stripe as the
   payment authority.
5. Remove SQL runtime dependencies only after rollback and restore drills pass.

### Phase D — decommissioning

SQL may be removed from the server deployment only when:

- all production routes use PADAGONIA clients;
- no runtime code imports PDO or executes SQL;
- the SQL database has been exported, checksummed, and archived according to
  the retention policy;
- restore from PADAGONIA snapshots succeeds on a clean host;
- a rollback rehearsal has been completed;
- the migration report is signed by the release process.

## 6. Superiority gate

PADAGONIA is allowed to claim superiority over SQL only if the same versioned
workloads are compared on the same hardware and dataset against SQLite and a
representative PostgreSQL deployment.

Required measurements:

- p50/p95/p99 latency for ingestion, dashboard reads, opportunity ranking,
  tenant-filtered traversal, and vector search;
- sustained writes per second and concurrent writer behavior;
- cold-start and warm query latency;
- bytes per record and total snapshot size;
- crash recovery time and acknowledged-write loss;
- backup and restore throughput;
- rebuild time from the event/outbox log;
- authorization and tenant-isolation tests;
- retention deletion and compaction cost;
- correctness equivalence across all migrated read models.

The replacement must meet all of these gates:

1. No cross-tenant read or write in adversarial tests.
2. Zero acknowledged-write loss after simulated process or host crash within
   the declared durability boundary.
3. Deterministic replay produces byte- or semantically-equivalent state.
4. p95 latency is no worse than the SQL baseline for control-plane operations.
5. Graph-native opportunity and similarity queries are materially better than
   their SQL equivalents on the declared workloads.
6. Recovery and rebuild are operationally documented and repeatedly tested.
7. No unbounded query, storage, or memory path exists at an authenticated API
   boundary.

If PADAGONIA wins graph intelligence but loses transactional control-plane
requirements, the correct architecture is hybrid—not a forced SQL removal.

## 7. Immediate implementation sequence

The next PADAGONIA milestones should be:

1. `NamespaceId`, stable external IDs, and namespace-aware storage validation.
2. Transaction journal with batch commit, replay, and idempotency records.
3. Secondary indexes and cursor pagination for status/time/priority queries.
4. Scoped roles, key rotation, per-tenant quotas, and authorization tests.
5. Retraction, retention, tombstone, deletion, and compaction primitives.
6. Query aggregates and a versioned HTTP client contract.
7. Dreamsequence outbox projector and shadow-read comparison harness.
8. Crash, restore, replay, tenant-isolation, and SQL-baseline benchmarks.

Every milestone must preserve `cargo fmt --check`, strict Clippy, the complete
test suite, storage compatibility fixtures, and the Kaptaind test gate.

## 8. Decision

PADAGONIA should become Dreamsequence's graph-native intelligence and, subject
to the gates above, its full server-side database. The project should not claim
that graph storage is inherently superior to SQL. It should demonstrate that
PADAGONIA is superior for this workload because it combines durable
transactional behavior with first-class provenance, relationship queries,
similarity search, and capability lineage that would otherwise be spread over
relational tables, joins, search indexes, and application-specific projections.
