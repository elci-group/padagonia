"""Deterministic power-law-ish workload generator matching padagonia bench_support."""
import csv
import sys
import random
import math

NODE_LABELS = ["Person", "Company", "Topic"]
RELATIONS = ["works_for", "knows", "located_in", "related_to"]


def splitmix64(seed: int):
    state = seed & 0xFFFFFFFFFFFFFFFF
    while True:
        state = (state + 0x9E3779B97F4A7C15) & 0xFFFFFFFFFFFFFFFF
        z = state
        z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9 & 0xFFFFFFFFFFFFFFFF
        z = (z ^ (z >> 27)) * 0x94D049BB133111EB & 0xFFFFFFFFFFFFFFFF
        yield z ^ (z >> 31)


def generate(nodes: int, edges: int, seed: int, out_dir: str = "."):
    rng = splitmix64(seed)

    def next_u32():
        return (next(rng) >> 32) & 0xFFFFFFFF

    def next_f():
        return next_u32() / 0xFFFFFFFF

    def next_usize(m):
        return next(rng) % m if m else 0

    def powerlaw_index(n, alpha=2.0):
        u = next_f()
        x = (1.0 - u) ** (-1.0 / (alpha - 1.0))
        return int(x) % n if n else 0

    with open(f"{out_dir}/nodes.csv", "w", newline="") as nf:
        nwriter = csv.writer(nf)
        nwriter.writerow(["id", "label", "name", "score", "age"])
        for i in range(nodes):
            label = NODE_LABELS[next_usize(len(NODE_LABELS))]
            nwriter.writerow([i, label, f"node_{i}", next_u32() % 1000, (next_u32() % 80) + 18])

    with open(f"{out_dir}/edges.csv", "w", newline="") as ef:
        ewriter = csv.writer(ef)
        ewriter.writerow(["id", "src", "dst", "label", "since", "confidence"])
        edge_id = 0
        for _ in range(edges):
            while True:
                src = powerlaw_index(nodes)
                dst = powerlaw_index(nodes)
                if src != dst:
                    break
            rel = RELATIONS[next_usize(len(RELATIONS))]
            since = (next_u32() % 30) + 1990
            conf = 0.5 + next_f() * 0.5
            ewriter.writerow([edge_id, src, dst, rel, since, f"{conf:.6f}"])
            edge_id += 1


if __name__ == "__main__":
    nodes = int(sys.argv[1]) if len(sys.argv) > 1 else 100_000
    edges = int(sys.argv[2]) if len(sys.argv) > 2 else 500_000
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 42
    out = sys.argv[4] if len(sys.argv) > 4 else "."
    generate(nodes, edges, seed, out)
    print(f"generated {nodes} nodes / {edges} edges in {out}")
