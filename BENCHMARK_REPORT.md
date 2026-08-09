# PADAGONIA Benchmark Report

## Workload
- Nodes: 100000
- Edges: 500000
- Facts: 600000
- Labels: 3
- Relations: 4
- PADAGONIA file size: 150,174,349 bytes

## PADAGONIA Results

| Metric | Value |
|---|---|
| Ingest | 605.9 ms (990.33 K items/s) |
| Save | 1.487 s |
| Load (parallel) | 1.555 s (385.91 K items/s) |
| Load (sequential) | 1.906 s |
| BFS depth 4 | 30.5 ms |
| Filter by relation | 2.9 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| networkx | 507.0 ms | 1.18 M items/s | 1.6 ms |
| sqlite | 1.393 s | 430.81 K items/s | 505.6 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 15.453 s | 1.1 ms | 0.842 |
| numpy_brute_force | 0.0 ms | 7.7 ms | 1.000 |
| hnswlib | 4.803 s | 0.7 ms | 0.856 |

## Observations

- **Ingest**: networkx is 0.84x PADAGONIA ingest time.
- **BFS**: networkx is 0.05x PADAGONIA BFS time.
- **Ingest**: sqlite is 2.30x PADAGONIA ingest time.
- **BFS**: sqlite is 16.59x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 7.21x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.63x PADAGONIA HNSW query latency.
