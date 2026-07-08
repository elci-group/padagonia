"""Read PADAGONIA and competitor results, generate BENCHMARK_REPORT.md."""
import json
from pathlib import Path


def fmt_seconds(s):
    if s < 1:
        return f"{s*1000:.1f} ms"
    return f"{s:.3f} s"


def fmt_tput(v):
    if v >= 1_000_000:
        return f"{v/1_000_000:.2f} M items/s"
    if v >= 1_000:
        return f"{v/1_000:.2f} K items/s"
    return f"{v:.2f} items/s"


def main():
    pad_file = Path("target/padagonia_bench_summary.json")
    hnsw_file = Path("target/padagonia_hnsw_summary.json")
    comp_file = Path("bench/competitors/results.json")
    report = Path("BENCHMARK_REPORT.md")

    pad = json.loads(pad_file.read_text()) if pad_file.exists() else {}
    hnsw = json.loads(hnsw_file.read_text()) if hnsw_file.exists() else {}
    comps = json.loads(comp_file.read_text()) if comp_file.exists() else []

    lines = []
    lines.append("# PADAGONIA Benchmark Report")
    lines.append("")
    lines.append("## Workload")
    lines.append(f"- Nodes: {pad.get('nodes', 'N/A')}")
    lines.append(f"- Edges: {pad.get('edges', 'N/A')}")
    lines.append(f"- Facts: {pad.get('facts', 'N/A')}")
    lines.append(f"- Labels: {pad.get('labels', 'N/A')}")
    lines.append(f"- Relations: {pad.get('relations', 'N/A')}")
    lines.append(f"- PADAGONIA file size: {pad.get('file_bytes', 0):,} bytes")
    lines.append("")

    lines.append("## PADAGONIA Results")
    lines.append("")
    lines.append("| Metric | Value |")
    lines.append("|---|---|")
    if pad:
        lines.append(f"| Ingest | {fmt_seconds(pad.get('ingest_seconds', 0))} ({fmt_tput(pad.get('ingest_throughput_items_per_sec', 0))}) |")
        lines.append(f"| Save | {fmt_seconds(pad.get('save_seconds', 0))} |")
        lines.append(f"| Load (parallel) | {fmt_seconds(pad.get('load_seconds', 0))} ({fmt_tput(pad.get('load_throughput_items_per_sec', 0))}) |")
        lines.append(f"| Load (sequential) | {fmt_seconds(pad.get('load_seq_seconds', 0))} |")
        lines.append(f"| BFS depth 4 | {fmt_seconds(pad.get('bfs_seconds', 0))} |")
        lines.append(f"| Filter by relation | {fmt_seconds(pad.get('filter_by_relation_seconds', 0))} |")
    lines.append("")

    lines.append("## Competitor Results")
    lines.append("")
    lines.append("| Competitor | Ingest | Ingest Throughput | BFS depth 4 |")
    lines.append("|---|---|---|---|")
    for c in comps:
        if "ingest_seconds" in c and "bfs_seconds" in c:
            lines.append(
                f"| {c.get('competitor', '?')} | "
                f"{fmt_seconds(c.get('ingest_seconds', 0))} | "
                f"{fmt_tput(c.get('ingest_throughput_items_per_sec', 0))} | "
                f"{fmt_seconds(c.get('bfs_seconds', 0))} |"
            )
    lines.append("")

    lines.append("## Vector Search")
    lines.append("")
    if hnsw:
        lines.append(f"Workload: {hnsw.get('vectors', 'N/A')} vectors × {hnsw.get('dim', 'N/A')} dims, k={hnsw.get('k', 'N/A')}, ef={hnsw.get('ef', 'N/A')}")
        lines.append("")
    lines.append("| Competitor | Build | Search/query | Recall@k |")
    lines.append("|---|---|---|---|")
    if hnsw:
        lines.append(
            f"| PADAGONIA HNSW | {fmt_seconds(hnsw.get('build_seconds', 0))} | "
            f"{fmt_seconds(hnsw.get('search_seconds', 0))} | "
            f"{hnsw.get('recall', 0):.3f} |"
        )
    for c in comps:
        if "search_seconds" in c and "recall" in c:
            lines.append(
                f"| {c.get('competitor', '?')} | "
                f"{fmt_seconds(c.get('build_seconds', 0))} | "
                f"{fmt_seconds(c.get('search_seconds', 0))} | "
                f"{c.get('recall', 0):.3f} |"
            )
    lines.append("")

    lines.append("## Observations")
    lines.append("")
    if pad and comps:
        pad_ingest = pad.get('ingest_seconds', 0)
        pad_bfs = pad.get('bfs_seconds', 0)
        for c in comps:
            name = c.get('competitor', '?')
            c_ingest = c.get('ingest_seconds', 0)
            c_bfs = c.get('bfs_seconds', 0)
            if c_ingest > 0:
                ratio = c_ingest / pad_ingest if pad_ingest > 0 else float('inf')
                lines.append(f"- **Ingest**: {name} is {ratio:.2f}x PADAGONIA ingest time.")
            if c_bfs > 0:
                ratio = c_bfs / pad_bfs if pad_bfs > 0 else float('inf')
                lines.append(f"- **BFS**: {name} is {ratio:.2f}x PADAGONIA BFS time.")
    else:
        lines.append("- Missing results; run benchmarks first.")

    if hnsw:
        lines.append("")
        lines.append("### Vector search notes")
        pad_search = hnsw.get('search_seconds', 0)
        for c in comps:
            if c.get('competitor') in {'numpy_brute_force', 'hnswlib'} and 'search_seconds' in c:
                ratio = c.get('search_seconds', 0) / pad_search if pad_search > 0 else float('inf')
                lines.append(
                    f"- **{c.get('competitor')}** query latency is {ratio:.2f}x PADAGONIA HNSW query latency."
                )
    lines.append("")

    report.write_text("\n".join(lines))
    print(f"wrote {report}")


if __name__ == "__main__":
    main()
