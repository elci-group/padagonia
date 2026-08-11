#!/usr/bin/env python3
"""Run the shared graph workload against a local PostgreSQL instance."""

import csv
import json
import subprocess
import sys
import time
from pathlib import Path


def psql(url: str, sql: str) -> str:
    result = subprocess.run(
        ["psql", url, "-v", "ON_ERROR_STOP=1", "-At", "-c", sql],
        check=True,
        text=True,
        capture_output=True,
    )
    return result.stdout.strip()


def main() -> None:
    out_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(".")
    url = sys.argv[2] if len(sys.argv) > 2 else "postgresql://postgres:padagonia-bench@127.0.0.1:55432/postgres"
    nodes = list(csv.DictReader((out_dir / "nodes.csv").open(newline="")))
    edges = list(csv.DictReader((out_dir / "edges.csv").open(newline="")))
    psql(url, "DROP TABLE IF EXISTS edges; DROP TABLE IF EXISTS nodes; CREATE TABLE nodes(id bigint PRIMARY KEY, label text, name text, score bigint, age bigint); CREATE TABLE edges(id bigint PRIMARY KEY, src bigint, dst bigint, label text, since bigint, confidence double precision); CREATE INDEX ON edges(src); CREATE INDEX ON edges(dst); CREATE INDEX ON edges(label);")
    node_file = out_dir / "nodes.pg.csv"
    edge_file = out_dir / "edges.pg.csv"
    node_file.write_text("".join(f"{r['id']}\t{r['label']}\t{r['name']}\t{r['score']}\t{r['age']}\n" for r in nodes))
    edge_file.write_text("".join(f"{r['id']}\t{r['src']}\t{r['dst']}\t{r['label']}\t{r['since']}\t{r['confidence']}\n" for r in edges))
    start = time.perf_counter()
    psql(url, f"\\copy nodes FROM '{node_file.resolve()}' WITH (FORMAT text)")
    psql(url, f"\\copy edges FROM '{edge_file.resolve()}' WITH (FORMAT text)")
    ingest = time.perf_counter() - start
    start = time.perf_counter()
    reached = psql(url, "WITH RECURSIVE bfs(node, depth) AS (SELECT (SELECT src FROM edges LIMIT 1), 0 UNION SELECT edges.dst, bfs.depth + 1 FROM bfs JOIN edges ON bfs.node = edges.src WHERE bfs.depth < 4) SELECT count(DISTINCT node) FROM bfs;")
    bfs = time.perf_counter() - start
    result = {"competitor": "postgresql", "nodes": len(nodes), "edges": len(edges), "ingest_seconds": ingest, "ingest_throughput_items_per_sec": (len(nodes) + len(edges)) / ingest, "bfs_seconds": bfs, "bfs_reached": int(reached)}
    results_file = out_dir / "results.json"
    entries = json.loads(results_file.read_text()) if results_file.exists() else []
    entries = [entry for entry in entries if entry.get("competitor") != "postgresql"]
    entries.append(result)
    results_file.write_text(json.dumps(entries, indent=2))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
