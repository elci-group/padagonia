#!/usr/bin/env python3
"""Deterministic CI gate for PADAGONIA benchmark artifacts."""

import json
import pathlib
import sys


def load(path: str) -> dict:
    with pathlib.Path(path).open(encoding="utf-8") as handle:
        return json.load(handle)


def fail(message: str) -> None:
    print(f"benchmark gate failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    if len(sys.argv) != 4:
        fail("usage: check-benchmark.py BASELINE GRAPH_SUMMARY VECTOR_SUMMARY")

    baseline = load(sys.argv[1])
    graph = load(sys.argv[2])
    vector = load(sys.argv[3])
    graph_policy = baseline["graph"]
    vector_policy = baseline["vector"]

    for key in ("nodes", "edges"):
        if graph.get(key) != graph_policy[key]:
            fail(f"graph {key} was {graph.get(key)!r}, expected {graph_policy[key]}")
    item_count = graph["nodes"] + graph["edges"]
    bytes_per_item = graph["file_bytes"] / item_count
    if bytes_per_item > graph_policy["max_bytes_per_item"]:
        fail(
            f"storage density {bytes_per_item:.2f} bytes/item exceeds "
            f"{graph_policy['max_bytes_per_item']:.2f}"
        )

    if vector.get("vectors") != vector_policy["vectors"]:
        fail("vector workload size does not match the baseline")
    if vector.get("dim") != vector_policy["dimensions"]:
        fail("vector dimensions do not match the baseline")
    if vector.get("k") != vector_policy["k"] or vector.get("ef") != vector_policy["ef"]:
        fail("vector search parameters do not match the baseline")
    recall = vector.get("recall")
    if not isinstance(recall, (int, float)) or recall < vector_policy["minimum_recall"]:
        fail(f"recall {recall!r} is below {vector_policy['minimum_recall']:.3f}")

    print(
        "benchmark gate passed: "
        f"{bytes_per_item:.2f} bytes/item, recall={recall:.3f}; "
        f"ingest={graph['ingest_throughput_items_per_sec']:.0f} items/s, "
        f"search={vector['search_seconds'] * 1000:.3f} ms/query (timings observational)"
    )


if __name__ == "__main__":
    main()
