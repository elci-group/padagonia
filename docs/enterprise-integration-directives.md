# Enterprise Integration Directives

This document is the implementation contract for using PADAGONIA as a shared
relationship, provenance, semantic-retrieval, and decision-history service.
Project roadmaps point here for the common controls and record their own
module-level delivery work separately.

## Enterprise baseline

Every integration MUST use a versioned namespace in the form
`<product>.<environment>.<tenant>` and MUST attach an immutable event or fact
provenance record containing producer, producer-version, source reference,
observed-at, asserted-at, confidence semantics, correlation ID, and retention
class. Caller-supplied confidence MUST NOT be treated as calibrated probability.

Every writer MUST use idempotency keys, bounded request sizes, bounded graph
growth, explicit timeouts, and retry handling that distinguishes accepted,
rejected, and unknown outcomes. A retry MUST be safe after a client timeout.

Every reader MUST tolerate missing, stale, or conflicting facts and MUST expose
the source and timestamp of material decisions. No integration may silently
overwrite a fact or use graph presence as proof of truth.

Production deployments MUST use TLS at the edge, secret injection rather than
checked-in keys, separate reader/writer/admin credentials, namespace quotas,
metrics, correlation IDs, audit logging, tested snapshots, and a documented
restore procedure. Sensitive payloads and credentials MUST remain outside the
graph unless a retention and erasure policy explicitly permits them.

The first implementation target is the existing single-node HTTP API. The
integration MUST remain deployable without replication. Replication, failover,
and multi-tenant adversarial isolation are release gates for shared managed
service claims, not assumptions that clients may make today.

## Common module contract

1. **Client module** — typed API client, API-version negotiation, bounded
   retries, idempotency, request correlation, auth injection, and stable error
   mapping.
2. **Domain mapping module** — deterministic mapping from project entities to
   labels, relations, properties, namespaces, and schema versions.
3. **Provenance module** — producer identity, source/evidence references,
   timestamps, confidence policy, content hashes, and redaction rules.
4. **Write pipeline** — transactional batches, backpressure, dead-letter
   handling, replay, and metrics for accepted/rejected/unknown writes.
5. **Read/retrieval module** — exact lookup, filtered traversal, aggregation,
   cursor pagination, vector search, freshness rules, and conflict handling.
6. **Cache/offline module** — local queue or read cache with bounded size,
   replay ordering, stale-read indicators, and safe degradation when Padagonia
   is unavailable.
7. **Security module** — least-privilege credentials, namespace enforcement,
   redaction, audit events, and secret-safe diagnostics.
8. **Operations module** — health/readiness, metrics, backup/restore, schema
   migration, retention, SLOs, and incident runbooks.
9. **Verification module** — contract tests against OpenAPI, property tests for
   serialization and retries, fixture graphs, migration tests, and load tests.

## Product directives

### DALM

Map corpus documents, chunks, claims, retrieval candidates, conflicts, and
answer citations to graph entities. Use vector-search for candidate recall and
explicit `supports`, `contradicts`, `derived_from`, and `supersedes` relations
for evidence reasoning. Keep ranking weights in DALM; Padagonia stores the
assertions and retrieval history. Add `padagonia_client`, `graph_writer`,
`evidence_reader`, `replay_queue`, and `contract_tests` modules. Gate release
on restart recovery, duplicate-ingest safety, tenant isolation, and citation
round-trip tests.

### Ami

Map projects, search hypotheses, sources, prospects, evidence signals, and
contact attempts into a durable market graph. Keep CSV as export, not the
system of record. Add typed client, prospect upsert writer, source-hash
deduplicator, confidence-policy module, and graph-backed `stats`/history
queries. Never store secrets or unverified personal data without retention
classification. Gate on deterministic reruns producing idempotent graph state.

### Mimic

Keep audio bytes and manifests on the filesystem/object store; store AudioUnit
metadata, embeddings, phonetic decomposition, `follows` edges, provider
provenance, and quality measurements in PADAGONIA. Add storage-admission,
unit-index, semantic-retrieval, composition-history, and recovery modules.
Require content hashes, immutable manifest references, bounded vector dimensions,
and atomic cache updates. Gate on cache correctness after restart, provider
failure recovery, and no orphaned manifest references.

### Dreamseq

Map sessions, normalized events, interventions, tool/model identities, patterns,
directives, and supporting evidence into a privacy-reduced graph. Add event
normalizer, redactor, batch writer, trend reader, directive lineage, and
retention modules. Store summaries and hashes rather than raw prompts by
default. Gate on cohort-only reporting, deletion/retention behavior, and
non-linkability of sensitive user content.

### GUAC and Coder by Vico

Represent repository state, prompt/task intent, tool calls, model outputs,
patches, tests, and outcomes as an execution graph. Add event adapter, code
entity mapper, provenance writer, semantic context reader, and replay verifier.
Use graph retrieval only as advisory context and show source links for every
injected fact. Gate on prompt-injection resistance, bounded context assembly,
and reproducible replay.

### EvidenceOS

Map evidence items, claims, controls, decisions, owners, deployments, and
reviews into an append-only evidence graph. Add evidence-ingest, hash/identity,
claim-linking, control-query, decision-export, and retention modules. Require
cryptographic content hashes even though PADAGONIA itself is not tamper-proof.
Gate on audit export, conflict visibility, and snapshot restore drills.

### Ferret and ProPro

Store providers, models, capabilities, pricing observations, benchmark runs,
routing decisions, and outcomes. Add provider registry, benchmark ingest,
scoring reader, cost aggregation, and decision-explanation modules. Keep
selection policy in the product and treat confidence as a ranking signal.
Gate on stale-price handling, reproducible benchmark provenance, and safe
fallback when the graph is unavailable.

### Fract and Traci

Map modules, symbols, dependencies, cycles, spans, violations, fixes, and
verification evidence. Add code-entity extraction, finding writer, trace
correlator, regression reader, and evidence export modules. Do not persist full
source or sensitive payloads by default. Gate on stable entity IDs, bounded
ingest, and correlation-preserving queries.

### Isopod and Spotcheck

Store controls, checks, assets, findings, evidence references, remediation
actions, and verification results. Add control ontology, scan ingest, finding
deduplication, exception lifecycle, and compliance export modules. Gate on
immutable audit history, explicit exception expiry, and reproducible reports.

### WriterAgent

Map documents, sections, citations, claims, revisions, and generated artifacts.
Add document entity mapper, citation writer, retrieval adapter, revision
lineage, redaction, and export modules. Keep document bodies in the existing
document store unless explicitly classified for graph storage. Gate on citation
provenance and rollback-safe revision history.

### Proteus, Skillastic, and Ontism

Use PADAGONIA for adaptive identity, skill lifecycle, and ontology state.
Add schema registry, entity mapper, lifecycle transaction writer, semantic
retrieval, and compatibility-query modules. Every identity or skill transition
must be an attributed assertion, never an in-place mutation. Gate on schema
versioning, conflict preservation, and deterministic replay.

### Turnsignal and Locksmith

Map agents, leases, routes, ownership, handoffs, and state transitions. Add
coordination-event writer, lease correlation, conflict reader, audit exporter,
and recovery modules. Keep the live lock decision in Locksmith; PADAGONIA is
the durable history and analysis layer. Gate on no false authority, idempotent
handoff recording, and bounded event retention.

### Vico Desktop, Vico View, and Vico WebUI

Use a shared typed client and expose project, agent, task, event, and graph
health views. Add client package, local gateway, namespace/session manager,
query hooks, offline queue, and operator diagnostics. UI must surface stale
data, source provenance, and authorization failures. Gate on local-only default
configuration, no credential leakage, and degraded operation when Padagonia is
down.

### Cheapwatch, Tokometer, Yield, Bound, Tight, Compactd, Bart, Deckhand,
### Amber, Baby, Boar, PV, Umami, Wired, and Jeenome

Use a common telemetry schema for sessions, files, resources, findings,
decisions, and outcomes. Add a small event adapter, bounded batch writer,
aggregation reader, and retention policy per project. Do not replace local
fast-path files or metrics; Padagonia is the cross-run history layer. Gate on
sampling, cardinality limits, and safe operation with the service unavailable.

### Hellhound and operational daemons

Store daemon identity, health observations, incidents, dependencies, and
remediation outcomes. Add health-event writer, incident correlator, fleet
query, and export modules. Keep alerting decisions local and fail closed for
unknown health. Gate on duplicate observation suppression and clock-skew
handling.

### SeriousCRM and Saints

Use Padagonia as a relationship/evidence layer beside the transactional store.
Map customers, contacts, interactions, teams, matches, players, events, and
source observations. Add CDC/event adapter, relationship query, semantic
search, and audit export modules. Do not make Padagonia the authoritative
ledger for financial or transactional mutations. Gate on reconciliation and
event replay.

### Vicount, Vitruvian, and Four-eyes

Map desktop entities, simulation components, visual tokens, render findings,
and generated artifacts. Add entity mapper, asset provenance, vector lookup,
and drift-history modules. Keep high-volume binary assets external. Gate on
stable IDs and bounded metadata growth.

## Enterprise acceptance gates

Each project integration is complete only when it has: a versioned schema;
typed client or adapter; idempotent writes; namespace and credential policy;
metrics and correlation IDs; offline/degraded behavior; contract and property
tests; fixture data with conflicts; migration notes; backup/restore ownership;
and a documented rollback. The project roadmap entry must link this document,
name its modules, and identify the owner, SLO, retention class, and release
gate.
