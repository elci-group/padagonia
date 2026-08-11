# PADAGONIA Benchmark Report

## Workload
- Nodes: 10000
- Edges: 50000
- Facts: 60000
- Labels: 3
- Relations: 4
- PADAGONIA file size: 14,842,245 bytes

## PADAGONIA Results

| Metric | Value |
|---|---|
| Ingest | 48.1 ms (1.25 M items/s) |
| Save | 95.2 ms |
| Load (parallel) | 131.0 ms (457.98 K items/s) |
| Load (sequential) | 147.5 ms |
| BFS depth 4 | 2.2 ms |
| Filter by relation | 0.2 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| postgresql | 315.3 ms | 190.32 K items/s | 99.2 ms |
| networkx | 45.9 ms | 1.31 M items/s | 0.6 ms |
| sqlite | 135.7 ms | 442.01 K items/s | 44.6 ms |

## Vector Search

Workload: 5000 vectors × 64 dims, k=10, ef=100

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 360.2 ms | 0.1 ms | 0.950 |
| numpy_brute_force | 0.0 ms | 8.3 ms | 1.000 |
| hnswlib | 9.948 s | 0.7 ms | 0.844 |

## Observations

- **Ingest**: postgresql is 6.55x PADAGONIA ingest time.
- **BFS**: postgresql is 44.17x PADAGONIA BFS time.
- **Ingest**: networkx is 0.95x PADAGONIA ingest time.
- **BFS**: networkx is 0.25x PADAGONIA BFS time.
- **Ingest**: sqlite is 2.82x PADAGONIA ingest time.
- **BFS**: sqlite is 19.85x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 78.93x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 6.60x PADAGONIA HNSW query latency.
