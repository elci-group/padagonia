# PADAGONIA Roadmap to SOTA Standards

PADAGONIA is a compact Rust graph store with ontology interning, immutable
provenance, parallel persistence, HNSW search, CLI tooling, a small HTTP server,
benchmarks, Docker packaging, and CI. The target is SOTA operational quality:
correct storage evolution, explicit security posture, measurable performance,
reproducible releases, and APIs that can be trusted by autonomous agent systems.

Current state: **B- open-source prototype**.

## Standards Bar

- Correctness: storage format changes are versioned, roundtrip-tested, fuzzed,
  and migration-aware.
- Performance: benchmark results are reproducible, trendable, and guarded
  against regressions.
- Security: dependencies are policy-gated, auth is scoped, mutations are
  auditable, and release artifacts are signed.
- Operations: server behavior is observable, configurable, bounded, and
  documented for self-hosting.
- Developer experience: CLI output is predictable, docs match code, CI is fast,
  and every release has a clear compatibility contract.
- Ecosystem: stable HTTP and client APIs allow agents, MCP servers, and other
  runtimes to build on PADAGONIA without knowing internal storage details.

## Phase 0 - Immediate Hardening (B- to B)

Goal: close release-readiness gaps found in the July 14, 2026 assessment.

- [x] Keep `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test` green.
- [x] Move server tracing/metrics initialization behind `padagonia server` so
  non-server CLI commands do not emit misleading server startup logs.
- [x] Update README storage documentation from `block.rs`/stale serializer notes
  to `storage.rs`/MessagePack.
- [x] Bump the on-disk storage format version after the serializer transition.
- [x] Remove currently unused direct dependencies from `Cargo.toml`.
- [x] Replace serializers with RustSec warnings with length-prefixed MessagePack
  frames.
- [x] Add explicit tests for loading old-format files or document that v1 files
  are intentionally incompatible.
- [x] Add a storage-corruption fixture test that verifies CRC and header errors
  are stable user-facing failures.

## Phase 1 - Storage Correctness (B to A-)

Goal: make data persistence dependable enough for production self-hosting.

- [x] Define a public storage compatibility policy in README and release notes.
- [ ] Introduce a migration layer keyed by storage version.
- [ ] Add golden binary fixtures for each supported storage version.
- [ ] Add property/fuzz tests for save/load, corrupted bytes, truncated files,
  large string tables, empty stores, and competing facts.
- [ ] Replace whole-file reads on load with bounded streaming decode or document
  memory limits.
- [ ] Add snapshot and restore CLI commands.
- [ ] Add a write-ahead log or equivalent durable mutation journal.

## Phase 2 - API And Operations (A- to A)

Goal: make the server safe and predictable under real operational use.

- [ ] Return structured error bodies for all HTTP failures.
- [ ] Add OpenAPI output and examples for `/api/v1/*`.
- [ ] Enforce request body size limits, timeout policy, and rate limiting.
- [ ] Add integration tests for health, metrics, auth, stats, ingest, and
  persistence reload behavior.
- [ ] Persist server mutations to disk or clearly mark the server as ephemeral.
- [ ] Add graceful shutdown tests for in-flight requests.
- [ ] Add Docker healthcheck and documented volume layout.

## Phase 3 - Security And Supply Chain (A to A+)

Goal: pass a serious security review.

- [ ] Add `cargo-deny` for license, duplicate, advisory, and yanked-crate policy.
- [ ] Replace plain API-key strings with secret-bearing types once the design
  justifies the dependency.
- [ ] Add scoped API keys and roles: admin, writer, reader.
- [ ] Add audit logs for auth failures and mutations.
- [ ] Add TLS guidance and a reverse-proxy deployment recipe.
- [ ] Generate SBOMs for release artifacts.
- [ ] Sign release artifacts and publish checksums.
- [ ] Expand `SECURITY.md` with threat model, supported versions, and disclosure
  workflow.

## Phase 4 - Performance Leadership (A+ to S)

Goal: make performance claims reproducible and defensible.

- [ ] Convert benchmark scripts into a single reproducible benchmark harness.
- [ ] Store machine-readable benchmark baselines under versioned artifacts.
- [ ] Gate major regressions in CI on representative small workloads.
- [ ] Add memory usage, file size, ingest throughput, load throughput, query
  latency, and HNSW recall metrics.
- [ ] Test sparse, dense, skewed, and adversarial graph shapes.
- [ ] Document hardware, dataset, and configuration for published numbers.
- [ ] Evaluate storage layout alternatives: streaming blocks, columnar blocks,
  mmap-friendly indexes, and compressed payloads.

## Phase 5 - Agent Ecosystem (S to S+)

Goal: become a reliable substrate for agentic systems.

- [ ] Stabilize a versioned HTTP API.
- [ ] Build Python and TypeScript clients with contract tests.
- [ ] Add MCP adapter for graph memory operations.
- [ ] Add import/export formats for common agent memory systems.
- [ ] Add multi-tenant namespace isolation.
- [ ] Add replication or backup shipping for managed deployments.
- [ ] Publish operational runbooks for backup, restore, upgrade, and incident
  response.

## Execution Rules

1. Every phase keeps `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test`, and relevant CLI smoke tests green.
2. User-facing behavior changes require README, changelog, and website updates.
3. Storage format changes require a version bump, fixtures, and migration notes.
4. New dependencies require an explicit reason and must pass audit/deny policy.
5. Performance claims require reproducible commands and machine-readable output.
6. Releases require clean git state, synced `Cargo.toml`/`Cargo.lock`/`VERSION`,
   generated checksums, and signed artifacts once signing is implemented.
