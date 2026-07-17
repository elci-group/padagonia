# Kimi Swarm Correctness Strategy

This strategy describes how to use a Kimi agent swarm to improve PADAGONIA
correctness without creating noisy overlap or unreviewable parallel edits.

## Objective

Raise correctness from "tested prototype" to "defensible database substrate" by
turning every risky behavior into one of:

- a documented contract,
- a deterministic regression test,
- a property/fuzz target,
- a migration fixture, or
- a rejected invalid state with a stable error.

## Swarm Topology

Use one lead agent and four bounded specialist lanes.

- Lead: owns task selection, conflict control, final integration, and release
  criteria.
- Storage lane: owns `src/storage.rs`, storage compatibility docs, binary
  fixtures, corruption handling, and migration tests.
- Query lane: owns `src/query.rs`, traversal invariants, fact semantics, and
  projection/query consistency checks.
- Vector lane: owns `src/hnsw.rs`, vector-search edge cases, dimensionality
  contracts, recall baselines, and deterministic index behavior.
- API/CLI lane: owns `src/cli.rs`, `src/server.rs`, config/auth behavior,
  command failure modes, and HTTP integration tests.

Agents must use disjoint write scopes. Explorers may inspect broadly but do not
edit. Workers edit only their assigned files and must not revert unowned changes.

## Operating Loop

1. Lead selects one correctness theme and writes acceptance criteria before
   spawning workers.
2. Explorers produce short risk inventories with file paths and proposed
   assertions.
3. Lead converts risks into small tasks with clear ownership and no overlapping
   write sets.
4. Workers add tests first, then implementation changes only where a failing
   test proves the behavior gap.
5. Lead integrates, runs the gate, updates docs/changelog/roadmap, and closes
   the loop.

## Correctness Backlog

### Storage

- Golden fixtures for every supported storage version.
- Explicit unsupported-version behavior.
- Rejection of trailing data, truncated frames, oversized frames, CRC mismatch,
  bad magic, and inconsistent block payload/kind pairs.
- Roundtrip equality for empty stores, competing facts, embeddings, sparse ids,
  labels/relations/properties, and high-cardinality string tables.
- Migration tests before supporting any old format.

### Query

- BFS depth boundaries, cycles, self-loops, missing starts, duplicate edges, and
  relation/confidence filters.
- `neighbors` symmetry/dedup ordering.
- `by_label`/`by_relation` index consistency against brute-force scans.
- `highest_confidence_fact` behavior for ties and non-finite confidence values.

### Vector

- Empty index, `k = 0`, `k > n`, missing embeddings, label filters with fewer
  than `k` candidates, and deterministic seeded builds.
- Explicit dimension mismatch contract.
- Exact-search comparator checks against brute force for tiny datasets.
- Recall baselines for representative seeded workloads.

### API/CLI

- CLI missing/invalid argument behavior.
- Server auth with empty API key, wrong bearer token, malformed header, and
  protected/public route separation.
- Persistence contract for server mutations.
- Structured error responses and body-size/timeout behavior.

## Gates

Every swarm batch must pass:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo audit`
- relevant CLI smoke tests
- `git diff --check`

The lead should not merge worker output that expands dependencies, changes the
storage format, or changes public API behavior without updating README,
CHANGELOG, ROADMAP, and the compatibility docs.

## Escalation Rules

- A storage format change requires a version bump and compatibility note.
- A newly accepted invalid input state requires a test explaining why it is safe.
- A newly rejected input state requires a stable error test.
- A nondeterministic test must be rewritten with a fixed seed or removed.
- A worker conflict is resolved by the lead, never by a worker reverting unknown
  changes.
