# Changelog

All notable changes to PADAGONIA will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial legal and governance files: `LICENSE-MIT`, `LICENSE-APACHE`,
  `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, and `CHANGELOG.md`.
- Storage-format tests covering current version stamping, old-version rejection,
  and CRC mismatch failures.
- Kimi swarm correctness strategy and storage compatibility policy docs.
- Atomic, fsync-backed graph replacement with validated `snapshot` and
  `restore` commands and failure-cleanup regression tests.
- Centralized HTTP limits for bodies, timeouts, request rate/burst, ingest,
  traversal depth, vector dimensions, result count, and search effort.
- Request correlation IDs, structured authentication/mutation/persistence
  events, a served OpenAPI 3.1 contract, threat model, design principles, and
  evidence-backed SOTA assessment.
- MSRV and rustdoc CI gates plus release checksums, CycloneDX SBOMs, GitHub
  build-provenance attestations, and Linux ARM64 artifacts.

### Changed

- Documented the SOTA maturity roadmap and immediate release-readiness gates.
- Moved server tracing and metrics initialization behind the `server` command so
  non-server CLI commands stay quiet.
- Bumped the PADAGONIA storage format version to 2 after formalizing the
  storage compatibility boundary.
- Switched storage serialization to length-prefixed MessagePack frames after
  audit checks showed both `postcard` and `bincode` carried unmaintained
  dependency warnings.
- Tightened storage loading to reject oversized frames and trailing bytes after
  the declared block count.
- Added storage validation for block kind/payload consistency, ontology id
  resolution, dangling edges, and semantic roundtrip indexes.
- Changed BFS on a missing start node to return an empty result instead of
  reporting the nonexistent node as reached.
- Replaced aspirational ontology/immutability language with precise implemented
  guarantees and explicit non-goals.
- Made Docker deployment fail closed unless an API key is injected through the
  environment.
- Removed production panic and silently discarded error paths from CLI,
  metrics, storage cleanup, and shutdown handling.

### Removed

- Removed the unused `secrecy` dependency until API keys are represented with
  secret wrapper types.

## [0.1.3] - 2026-07-09

### Added

- Phase 1 dependencies for server, config, observability, and metrics.
- `README.md` documentation for build, CLI, benchmarks, and architecture.

## [0.1.0] - 2026-07-08

### Added

- Initial PADAGONIA prototype.
- Ontology-native graph store with interned labels, relations, and property keys.
- Immutable nodes and edges with provenance and confidence metadata.
- Parallel binary block format with CRC32 validation.
- Query engine with traversal, filtering, and BFS support.
- Native HNSW approximate nearest-neighbor search over embeddings.
- JSON/JSONL/CSV projections and deterministic synthetic benchmarks.
