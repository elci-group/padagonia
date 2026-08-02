# PADAGONIA Benchmark Report

## Method

The CI workload is deterministic and versioned in `bench/ci-baseline.json`.
Storage density and HNSW recall are enforced because they are stable semantic
metrics. Wall-clock throughput and latency are recorded as observations but are
not pass/fail gates on shared runners.

Reproduce the gate:

```bash
cargo run --locked --release -- bench --nodes 10000 --edges 50000 --seed 42
cargo run --locked --release -- bench-vectors --nodes 5000 --dim 64 --k 10 --ef 100 --m 16 --queries 20 --seed 123
python3 scripts/check-benchmark.py bench/ci-baseline.json \
  target/padagonia_bench_summary.json target/padagonia_hnsw_summary.json
```

## Reference observation (2026-07-31)

- CPU: Intel Core 5 120U, 10 cores / 12 threads, 12 MiB L3
- OS: Linux 6.18.7 x86-64
- Compiler: rustc 1.96.1, optimized release profile
- Graph: 10,000 nodes, 50,000 edges, seed 42
- Storage: 12,584,465 bytes, 209.74 bytes per node/edge
- Ingest: 2.03 million node/edge items per second
- Load: 536 thousand node/edge items per second
- BFS depth 4: 2.33 ms median across five runs
- Vector: 5,000 vectors, 64 dimensions, `k=10`, `ef=100`, 20 queries
- HNSW recall@10: 0.975 against brute force
- HNSW search: 0.101 ms/query average
- Brute-force search: 0.277 ms/query average

Power state, background load, allocator, kernel, and compiler affect timings;
these figures are not universal performance claims. The checked-in competitor
harness remains useful for comparative experiments, but a published comparison
must record dependency versions, identical workload input, hardware, warm-up,
run distribution, and uncertainty.

## Gate thresholds

- workload shape must exactly match the versioned baseline;
- serialized storage must stay at or below 260 bytes per node/edge item;
- HNSW recall@10 must remain at or above 0.90;
- all machine-readable summaries are retained as CI artifacts.
