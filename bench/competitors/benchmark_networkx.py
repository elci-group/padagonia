"""Benchmark NetworkX on the same deterministic workload."""
import csv
import json
import sys
import time
from pathlib import Path

try:
    import networkx as nx
except ImportError:
    print("NetworkX not installed; install with 'pip install networkx'")
    sys.exit(0)


def timed(func):
    start = time.perf_counter()
    result = func()
    return result, time.perf_counter() - start


def main():
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
    nodes_path = out_dir / "nodes.csv"
    edges_path = out_dir / "edges.csv"

    if not nodes_path.exists() or not edges_path.exists():
        print("nodes.csv/edges.csv not found; run generate_workload.py first")
        sys.exit(1)

    g = nx.DiGraph()
    with open(nodes_path, newline="") as f:
        reader = csv.DictReader(f)
        node_rows = list(reader)

    _, ingest_nodes_time = timed(lambda: [g.add_node(int(r["id"]), **r) for r in node_rows])

    with open(edges_path, newline="") as f:
        reader = csv.DictReader(f)
        edge_rows = list(reader)

    def add_edges():
        for r in edge_rows:
            g.add_edge(int(r["src"]), int(r["dst"]), label=r["label"], since=int(r["since"]))

    _, ingest_edges_time = timed(add_edges)
    ingest_time = ingest_nodes_time + ingest_edges_time

    start_node = next((n for n in g.nodes if g.out_degree(n) > 0), 0)
    _, bfs_time = timed(lambda: list(nx.bfs_edges(g, start_node, depth_limit=4)))

    result = {
        "competitor": "networkx",
        "nodes": len(node_rows),
        "edges": len(edge_rows),
        "ingest_seconds": ingest_time,
        "ingest_throughput_items_per_sec": (len(node_rows) + len(edge_rows)) / ingest_time,
        "bfs_seconds": bfs_time,
    }

    results_file = Path("bench/competitors/results.json")
    results_file.parent.mkdir(parents=True, exist_ok=True)
    entries = []
    if results_file.exists():
        entries = json.loads(results_file.read_text())
        entries = [e for e in entries if e.get("competitor") != "networkx"]
    entries.append(result)
    results_file.write_text(json.dumps(entries, indent=2))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
