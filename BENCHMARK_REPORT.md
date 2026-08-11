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
| Ingest | 587.2 ms (1.02 M items/s) |
| Save | 1.106 s |
| Load (parallel) | 1.539 s (389.80 K items/s) |
| Load (sequential) | 1.853 s |
| BFS depth 4 | 59.0 ms |
| Filter by relation | 2.9 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| postgresql | 315.3 ms | 190.32 K items/s | 99.2 ms |
| networkx | 50.9 ms | 1.18 M items/s | 0.6 ms |
| sqlite | 133.6 ms | 449.02 K items/s | 39.8 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 15.710 s | 1.1 ms | 0.840 |
| numpy_brute_force | 0.0 ms | 7.6 ms | 1.000 |
| hnswlib | 4.989 s | 0.5 ms | 0.849 |

## Observations

- **Ingest**: postgresql is 0.54x PADAGONIA ingest time.
- **BFS**: postgresql is 1.68x PADAGONIA BFS time.
- **Ingest**: networkx is 0.09x PADAGONIA ingest time.
- **BFS**: networkx is 0.01x PADAGONIA BFS time.
- **Ingest**: sqlite is 0.23x PADAGONIA ingest time.
- **BFS**: sqlite is 0.68x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 7.06x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.46x PADAGONIA HNSW query latency.
