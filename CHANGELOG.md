# Changelog

All notable changes to PADAGONIA will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial legal and governance files: `LICENSE-MIT`, `LICENSE-APACHE`,
  `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, and `CHANGELOG.md`.

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
