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
| Ingest | 572.3 ms (1.05 M items/s) |
| Save | 1.745 s |
| Load (parallel) | 1.703 s (352.41 K items/s) |
| Load (sequential) | 1.959 s |
| BFS depth 4 | 44.1 ms |
| Filter by relation | 2.8 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| networkx | 529.5 ms | 1.13 M items/s | 1.6 ms |
| sqlite | 1.382 s | 434.12 K items/s | 500.8 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 15.662 s | 1.0 ms | 0.822 |
| numpy_brute_force | 0.0 ms | 6.8 ms | 1.000 |
| hnswlib | 5.439 s | 0.6 ms | 0.854 |

## Observations

- **Ingest**: networkx is 0.93x PADAGONIA ingest time.
- **BFS**: networkx is 0.04x PADAGONIA BFS time.
- **Ingest**: sqlite is 2.41x PADAGONIA ingest time.
- **BFS**: sqlite is 11.36x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 6.45x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.60x PADAGONIA HNSW query latency.
