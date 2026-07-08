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
| Ingest | 1.581 s (379.58 K items/s) |
| Save | 3.812 s |
| Load (parallel) | 5.497 s (109.14 K items/s) |
| Load (sequential) | 7.221 s |
| BFS depth 4 | 401.2 ms |
| Filter by relation | 5.5 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| networkx | 1.233 s | 486.66 K items/s | 3.3 ms |
| sqlite | 2.913 s | 205.98 K items/s | 801.2 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 27.387 s | 1.6 ms | 0.832 |
| numpy_brute_force | 0.0 ms | 15.5 ms | 1.000 |
| hnswlib | 17.098 s | 1.1 ms | 0.849 |

## Observations

- **Ingest**: networkx is 0.78x PADAGONIA ingest time.
- **BFS**: networkx is 0.01x PADAGONIA BFS time.
- **Ingest**: sqlite is 1.84x PADAGONIA ingest time.
- **BFS**: sqlite is 2.00x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 9.98x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.70x PADAGONIA HNSW query latency.
