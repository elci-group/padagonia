"""Benchmark SQLite in-memory graph on the same workload."""
import csv
import json
import sqlite3
import sys
import time
from pathlib import Path


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

    with open(nodes_path, newline="") as f:
        node_rows = list(csv.DictReader(f))
    with open(edges_path, newline="") as f:
        edge_rows = list(csv.DictReader(f))

    conn = sqlite3.connect(":memory:")
    cur = conn.cursor()
    cur.executescript("""
        CREATE TABLE nodes(id INTEGER PRIMARY KEY, label TEXT, name TEXT, score INTEGER, age INTEGER);
        CREATE TABLE edges(id INTEGER PRIMARY KEY, src INTEGER, dst INTEGER, label TEXT, since INTEGER, confidence REAL);
        CREATE INDEX idx_edges_src ON edges(src);
        CREATE INDEX idx_edges_dst ON edges(dst);
        CREATE INDEX idx_edges_label ON edges(label);
    """)

    def ingest():
        cur.executemany(
            "INSERT INTO nodes(id, label, name, score, age) VALUES (?, ?, ?, ?, ?)",
            [(int(r["id"]), r["label"], r["name"], int(r["score"]), int(r["age"])) for r in node_rows],
        )
        cur.executemany(
            "INSERT INTO edges(id, src, dst, label, since, confidence) VALUES (?, ?, ?, ?, ?, ?)",
            [
                (int(r["id"]), int(r["src"]), int(r["dst"]), r["label"], int(r["since"]), float(r["confidence"]))
                for r in edge_rows
            ],
        )
        conn.commit()

    _, ingest_time = timed(ingest)

    cur.execute("SELECT src FROM edges LIMIT 1")
    row = cur.fetchone()
    start_node = row[0] if row else 0
    # Recursive CTE BFS up to depth 4
    bfs_sql = """
        WITH RECURSIVE bfs(node, depth) AS (
            SELECT ?, 0
            UNION
            SELECT edges.dst, bfs.depth + 1
            FROM bfs JOIN edges ON bfs.node = edges.src
            WHERE bfs.depth < ?
        )
        SELECT DISTINCT node FROM bfs;
    """

    def run_bfs():
        cur.execute(bfs_sql, (start_node, 4))
        return cur.fetchall()

    rows, bfs_time = timed(run_bfs)

    result = {
        "competitor": "sqlite",
        "nodes": len(node_rows),
        "edges": len(edge_rows),
        "ingest_seconds": ingest_time,
        "ingest_throughput_items_per_sec": (len(node_rows) + len(edge_rows)) / ingest_time,
        "bfs_seconds": bfs_time,
        "bfs_reached": len(rows),
    }

    results_file = Path("bench/competitors/results.json")
    results_file.parent.mkdir(parents=True, exist_ok=True)
    entries = []
    if results_file.exists():
        entries = json.loads(results_file.read_text())
        entries = [e for e in entries if e.get("competitor") != "sqlite"]
    entries.append(result)
    results_file.write_text(json.dumps(entries, indent=2))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
