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
| Ingest | 48.7 ms (1.23 M items/s) |
| Save | 100.6 ms |
| Load (parallel) | 130.7 ms (459.12 K items/s) |
| Load (sequential) | 147.3 ms |
| BFS depth 4 | 2.4 ms |
| Filter by relation | 0.2 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| postgresql | 315.3 ms | 190.32 K items/s | 99.2 ms |
| networkx | 56.1 ms | 1.07 M items/s | 0.6 ms |
| sqlite | 112.5 ms | 533.19 K items/s | 35.1 ms |

## Vector Search

Workload: 5000 vectors × 64 dims, k=10, ef=100

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 357.4 ms | 0.1 ms | 0.945 |
| numpy_brute_force | 0.0 ms | 8.3 ms | 1.000 |
| hnswlib | 7.087 s | 0.8 ms | 0.859 |

## Observations

- **Ingest**: postgresql is 6.48x PADAGONIA ingest time.
- **BFS**: postgresql is 41.55x PADAGONIA BFS time.
- **Ingest**: networkx is 1.15x PADAGONIA ingest time.
- **BFS**: networkx is 0.25x PADAGONIA BFS time.
- **Ingest**: sqlite is 2.31x PADAGONIA ingest time.
- **BFS**: sqlite is 14.72x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 79.08x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 7.48x PADAGONIA HNSW query latency.
