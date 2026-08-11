# PADAGONIA Benchmark Report

## Workload
- Nodes: 100000
- Edges: 500000
- Facts: 600000
- Labels: 3
- Relations: 4
- PADAGONIA file size: 149,574,349 bytes

## PADAGONIA Results

| Metric | Value |
|---|---|
| Ingest | 587.3 ms (1.02 M items/s) |
| Save | 1.075 s |
| Load (parallel) | 1.493 s (401.93 K items/s) |
| Load (sequential) | 1.922 s |
| BFS depth 4 | 30.5 ms |
| Filter by relation | 3.0 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| postgresql | 315.3 ms | 190.32 K items/s | 99.2 ms |
| networkx | 38.6 ms | 1.55 M items/s | 0.5 ms |
| sqlite | 132.1 ms | 454.20 K items/s | 40.7 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 16.020 s | 1.2 ms | 0.834 |
| numpy_brute_force | 0.0 ms | 8.1 ms | 1.000 |
| hnswlib | 5.016 s | 0.5 ms | 0.850 |

## Observations

- **Ingest**: postgresql is 0.54x PADAGONIA ingest time.
- **BFS**: postgresql is 3.25x PADAGONIA BFS time.
- **Ingest**: networkx is 0.07x PADAGONIA ingest time.
- **BFS**: networkx is 0.02x PADAGONIA BFS time.
- **Ingest**: sqlite is 0.22x PADAGONIA ingest time.
- **BFS**: sqlite is 1.33x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 6.67x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.44x PADAGONIA HNSW query latency.
