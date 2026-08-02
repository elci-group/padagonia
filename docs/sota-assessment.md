# State-of-the-Art Assessment

This is PADAGONIA's living gap analysis. It separates present evidence from
aspiration so that the project never treats a roadmap checkbox, benchmark, or
marketing phrase as proof of a production property.

## Intended outcome

PADAGONIA aims to be a dependable, ontology-aware graph substrate for agent
memory: append-only graph facts, explicit provenance, bounded persistence,
predictable query behavior, and operational interfaces that autonomous systems
can use without depending on Rust internals.

"State of the art" means that every material claim is attached to an
invariant, a test or measurement, an operating limit, and a recovery story. It
does not mean maximizing the feature count.

## Baseline evidence (2026-07-31)

- Rust 1.96.1 builds the crate; the manifest declares MSRV 1.85.
- Formatting, Clippy with warnings denied, the ordinary test suite, rustdoc,
  and the release build pass locally.
- 43 non-ignored tests cover serialization, corruption rejection, queries,
  vector recall, authentication, HTTP mutation, and write-through persistence.
- RustSec reports no known advisories in the resolved dependency graph.
- cargo-deny passes advisories, bans, licenses, and sources, with duplicate and
  stale-allowance warnings that remain debt.
- Traci reports 43 production observability findings: 9 critical opaque panic
  paths, 30 errors, and 4 warnings. The aggregate implementation-complexity
  routing score is 100; `server.rs` and `storage.rs` are the hottest modules.
- Amber finds no dependency that should be removed blindly. Several convenient
  crates score as replaceable, but replacing serialization, metrics, or the
  HNSW engine would increase correctness risk without evidence of a benefit.
- The repository is small and comprehensible (about 3,000 production Rust
  lines); `cli.rs`, `server.rs`, and `storage.rs` are its largest source files.
- Kaptaind is configured for gated builds, semantic versioning, push, and Linux
  binary shipping. Detached monitoring cannot bind its health port inside the
  current sandbox, so this session supervises it in the foreground.

## Gap between language and reality

PADAGONIA currently calls itself ontology-native, immutable, provenance-rich,
and designed for autonomous agents. The implementation supports interned
vocabulary, append-only node and edge values, attached provenance records, and
agent-friendly APIs. Those are necessary foundations, but they are not yet:

- ontology reasoning, constraint validation, or semantic entailment;
- immutable storage under a hostile operator or cryptographic history;
- verified provenance, calibrated confidence, or evidence integrity;
- tenant isolation, delegated authority, erasure policy, or agent identity;
- a distributed database with replication, consensus, or availability claims.

Documentation and product language must preserve those distinctions.

## Ranked improvement path

### P0 — Data integrity and recovery

Importance: critical. An agent-memory system that acknowledges a mutation and
loses or corrupts it creates false beliefs downstream.

Acceptance evidence:

- saves use a same-directory temporary file, flush data, atomically replace the
  destination, and sync the parent directory where the platform supports it;
- failed saves do not destroy the last known-good graph;
- snapshot and restore commands are explicit, tested, and documented;
- loading remains bounded per frame and rejects malformed semantic state;
- the durability model states exactly what is and is not guaranteed.

### P0 — Bounded, abuse-resistant API behavior

Importance: critical. Authentication without resource limits still allows an
authorized or compromised caller to exhaust memory, CPU, disk, and latency.

Acceptance evidence:

- request bodies, synthetic ingest size, BFS depth, vector dimensions, `k`,
  and search effort have configured upper bounds;
- timeouts and rate limits return structured error bodies;
- unsafe defaults (blank keys and unbounded public bind) fail closed;
- integration tests pin public/protected route behavior and each limit.

### P0 — Reconstructable failures and auditability

Importance: critical. Provenance-rich data with opaque runtime failures is not
operationally trustworthy.

Acceptance evidence:

- production `unwrap`/`expect` and silently discarded results are removed;
- authentication failures and accepted mutations emit structured events with
  operation and object identifiers but never credentials;
- request correlation identifiers flow through HTTP traces;
- Traci's critical findings are zero and remaining exceptions are justified.

### P1 — Epistemic and ethical contract

Importance: high. The data model encodes claims about truth, authority,
identity, and permanence; leaving them implicit is a design defect.

Acceptance evidence:

- principles define observations versus assertions, competing claims,
  confidence semantics, evidence references, authority, and non-goals;
- immutability is reconciled with correction, redaction, retention, and the
  limits of erasure in backups;
- the threat model covers callers, operators, dependencies, data poisoning,
  denial of service, leakage, and recovery.

### P1 — Stable and discoverable API contract

Importance: high. Agent runtimes need a machine-readable contract independent
of implementation details.

Acceptance evidence:

- an OpenAPI 3.1 document describes every route, auth scheme, limit, request,
  response, and structured error;
- a served endpoint exposes the exact checked-in contract;
- compatibility and deprecation rules are explicit and contract-tested.

### P1 — Supply-chain and release provenance

Importance: high. Checksums alone detect corruption but do not prove where an
artifact came from.

Acceptance evidence:

- CI runs formatting, Clippy, tests, rustdoc, audit, deny, and MSRV checks;
- release artifacts include SHA-256 checksums and an SBOM;
- GitHub build provenance attestations bind artifacts to workflow and commit;
- workflow permissions are least-privilege and third-party actions are pinned.

### P2 — Defensible performance

Importance: medium-high. Speed claims without recall, memory, distributions,
hardware, and regression thresholds are anecdotes.

Acceptance evidence:

- a deterministic small benchmark produces versioned machine-readable metrics;
- CI compares against explicit tolerances without relying on noisy wall-clock
  microbenchmarks alone;
- published reports include hardware, compiler, workload shape, file size,
  throughput, latency percentiles, memory, and HNSW recall.

### P2 — Architecture boundaries

Importance: medium. The codebase is still small, but server, CLI, and storage
concentrate change risk.

Acceptance evidence:

- persistence, policy limits, API models, and process lifecycle have distinct
  modules with narrow interfaces;
- architectural health is measured when the Fract profiler is available;
- complexity decreases without gratuitous abstraction or dependency growth.

### P3 — Ecosystem and distributed operation

Importance: strategic, after the substrate gates above.

Acceptance evidence:

- generated Python and TypeScript clients pass contract tests;
- an MCP adapter maps graph-memory operations without bypassing authorization;
- namespaces isolate tenant data and quotas;
- backup shipping and replication have explicit consistency and recovery-point
  objectives before being described as highly available.

## Decision rules

1. Correctness and recovery outrank feature breadth and benchmark wins.
2. A failing test is useful evidence; suppressing a diagnostic is not a fix.
3. No dependency is removed or added solely to improve an automated score.
4. Storage and HTTP compatibility changes require fixtures and release notes.
5. Security boundaries fail closed, preserve credential secrecy, and are
   exercised from the caller's perspective.
6. Performance claims include reproducible inputs and uncertainty.
7. Aspirational distributed features do not block honest, high-quality
   single-node operation, but they may not be implied before they exist.
