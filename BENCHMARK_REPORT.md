# PADAGONIA Benchmark Report

## Workload
- Nodes: 10000
- Edges: 50000
- Facts: 60000
- Labels: 3
- Relations: 4
- PADAGONIA file size: 14,782,245 bytes

## PADAGONIA Results

| Metric | Value |
|---|---|
| Ingest | 49.6 ms (1.21 M items/s) |
| Save | 769.7 ms |
| Load (parallel) | 142.6 ms (420.72 K items/s) |
| Load (sequential) | 153.4 ms |
| BFS depth 4 | 2.3 ms |
| Filter by relation | 0.2 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| postgresql | 315.3 ms | 190.32 K items/s | 99.2 ms |
| networkx | 43.4 ms | 1.38 M items/s | 0.5 ms |
| sqlite | 129.4 ms | 463.75 K items/s | 40.0 ms |

## Vector Search

Workload: 5000 vectors × 64 dims, k=10, ef=100

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 272.1 ms | 0.1 ms | 0.945 |
| numpy_brute_force | 0.0 ms | 7.7 ms | 1.000 |
| hnswlib | 4.735 s | 0.4 ms | 0.848 |

## Observations

- **Ingest**: postgresql is 6.36x PADAGONIA ingest time.
- **BFS**: postgresql is 42.95x PADAGONIA BFS time.
- **Ingest**: networkx is 0.88x PADAGONIA ingest time.
- **BFS**: networkx is 0.23x PADAGONIA BFS time.
- **Ingest**: sqlite is 2.61x PADAGONIA ingest time.
- **BFS**: sqlite is 17.34x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 98.92x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 5.60x PADAGONIA HNSW query latency.
