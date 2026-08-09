# PADAGONIA Benchmark Report

## Workload
- Nodes: 100000
- Edges: 500000
- Facts: 600000
- Labels: 3
- Relations: 4
- PADAGONIA file size: 150,774,349 bytes

## PADAGONIA Results

| Metric | Value |
|---|---|
| Ingest | 610.7 ms (982.42 K items/s) |
| Save | 1.535 s |
| Load (parallel) | 1.517 s (395.50 K items/s) |
| Load (sequential) | 2.143 s |
| BFS depth 4 | 73.5 ms |
| Filter by relation | 2.8 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| networkx | 528.2 ms | 1.14 M items/s | 1.5 ms |
| sqlite | 1.382 s | 434.29 K items/s | 499.1 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 15.461 s | 1.1 ms | 0.846 |
| numpy_brute_force | 0.0 ms | 6.8 ms | 1.000 |
| hnswlib | 4.676 s | 0.5 ms | 0.855 |

## Observations

- **Ingest**: networkx is 0.86x PADAGONIA ingest time.
- **BFS**: networkx is 0.02x PADAGONIA BFS time.
- **Ingest**: sqlite is 2.26x PADAGONIA ingest time.
- **BFS**: sqlite is 6.79x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 6.07x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.41x PADAGONIA HNSW query latency.
