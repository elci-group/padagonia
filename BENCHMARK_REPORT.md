# PADAGONIA Benchmark Report

## Workload
- Nodes: 100000
- Edges: 500000
- Facts: 600000
- Labels: 3
- Relations: 4
- PADAGONIA file size: 124,989,561 bytes

## PADAGONIA Results

| Metric | Value |
|---|---|
| Ingest | 1.005 s (597.05 K items/s) |
| Save | 2.247 s |
| Load (parallel) | 3.366 s (178.26 K items/s) |
| Load (sequential) | 3.935 s |
| BFS depth 4 | 241.6 ms |
| Filter by relation | 4.2 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| networkx | 1.249 s | 480.57 K items/s | 3.6 ms |
| sqlite | 2.020 s | 296.96 K items/s | 623.5 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 62.907 s | 1.2 ms | 0.894 |
| numpy_brute_force | 0.0 ms | 18.4 ms | 1.000 |
| hnswlib | 13.732 s | 1.1 ms | 0.851 |

## Observations

- **Ingest**: networkx is 1.24x PADAGONIA ingest time.
- **BFS**: networkx is 0.01x PADAGONIA BFS time.
- **Ingest**: sqlite is 2.01x PADAGONIA ingest time.
- **BFS**: sqlite is 2.58x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 15.19x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.89x PADAGONIA HNSW query latency.
