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
| Ingest | 506.1 ms (1.19 M items/s) |
| Save | 8.002 s |
| Load (parallel) | 1.217 s (493.04 K items/s) |
| Load (sequential) | 1.752 s |
| BFS depth 4 | 53.9 ms |
| Filter by relation | 5.2 ms |

## Competitor Results

| Competitor | Ingest | Ingest Throughput | BFS depth 4 |
|---|---|---|---|
| networkx | 730.5 ms | 821.38 K items/s | 2.1 ms |
| sqlite | 1.821 s | 329.55 K items/s | 578.5 ms |

## Vector Search

Workload: 50000 vectors × 128 dims, k=10, ef=200

| Competitor | Build | Search/query | Recall@k |
|---|---|---|---|
| PADAGONIA HNSW | 19.517 s | 1.0 ms | 0.812 |
| numpy_brute_force | 0.0 ms | 12.7 ms | 1.000 |
| hnswlib | 7.675 s | 0.6 ms | 0.852 |

## Observations

- **Ingest**: networkx is 1.44x PADAGONIA ingest time.
- **BFS**: networkx is 0.04x PADAGONIA BFS time.
- **Ingest**: sqlite is 3.60x PADAGONIA ingest time.
- **BFS**: sqlite is 10.72x PADAGONIA BFS time.

### Vector search notes
- **numpy_brute_force** query latency is 12.24x PADAGONIA HNSW query latency.
- **hnswlib** query latency is 0.53x PADAGONIA HNSW query latency.
