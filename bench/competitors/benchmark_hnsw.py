"""Benchmark vector ANN: brute-force numpy and hnswlib vs PADAGONIA HNSW."""
import csv
import json
import sys
import time
from pathlib import Path

import numpy as np

try:
    import hnswlib
except ImportError:
    hnswlib = None


def timed(func):
    start = time.perf_counter()
    result = func()
    return result, time.perf_counter() - start


def load_vectors(path: Path):
    with open(path, newline="") as f:
        reader = csv.reader(f)
        header = next(reader)
        dim = len(header) - 1
        rows = [[float(x) for x in row[1:]] for row in reader]
    return np.array(rows, dtype=np.float32), dim


def main():
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
    vectors_path = out_dir / "vectors.csv"
    if not vectors_path.exists():
        print("vectors.csv not found; run generate_vectors.py first")
        sys.exit(1)

    X, dim = load_vectors(vectors_path)
    N = X.shape[0]
    K = 10
    M = 24
    EF_CONSTRUCTION = 200
    EF_SEARCH = 200
    queries = 100
    rng = np.random.default_rng(7)
    Q = rng.random((queries, dim), dtype=np.float32)

    # ---- brute force ----
    def brute_search():
        results = []
        for q in Q:
            dists = np.sum((X - q) ** 2, axis=1)
            topk = np.argpartition(dists, K - 1)[:K]
            # sort top-k for stable comparison
            order = np.argsort(dists[topk])
            results.append(set(topk[order].tolist()))
        return results

    brute_results, brute_time = timed(brute_search)
    brute_avg = brute_time / queries

    entries = [
        {
            "competitor": "numpy_brute_force",
            "vectors": N,
            "dim": dim,
            "k": K,
            "build_seconds": 0.0,
            "search_seconds": brute_avg,
            "recall": 1.0,
        }
    ]

    # ---- hnswlib ----
    if hnswlib is not None:
        space = "l2"
        idx = hnswlib.Index(space=space, dim=dim)
        idx.init_index(max_elements=N, ef_construction=EF_CONSTRUCTION, M=M, random_seed=123)

        _, build_time = timed(lambda: idx.add_items(X, np.arange(N, dtype=np.int64)))
        idx.set_ef(EF_SEARCH)

        def hnsw_search():
            results = []
            for q in Q:
                labels, _ = idx.knn_query(q, k=K)
                results.append(set(labels[0].tolist()))
            return results

        hnsw_results, hnsw_time = timed(hnsw_search)
        hnsw_avg = hnsw_time / queries

        hits = sum(len(a & b) for a, b in zip(brute_results, hnsw_results))
        recall = hits / (queries * K)

        entries.append(
            {
                "competitor": "hnswlib",
                "vectors": N,
                "dim": dim,
                "k": K,
                "build_seconds": build_time,
                "search_seconds": hnsw_avg,
                "recall": recall,
            }
        )

    results_file = Path("bench/competitors/results.json")
    results_file.parent.mkdir(parents=True, exist_ok=True)
    all_entries = []
    if results_file.exists():
        all_entries = json.loads(results_file.read_text())
    # remove previous vector competitor entries
    all_entries = [
        e
        for e in all_entries
        if e.get("competitor") not in {"numpy_brute_force", "hnswlib"}
    ]
    all_entries.extend(entries)
    results_file.write_text(json.dumps(all_entries, indent=2))

    for e in entries:
        print(json.dumps(e, indent=2))


if __name__ == "__main__":
    main()
