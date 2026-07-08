"""Generate a deterministic vector workload for ANN benchmarking."""
import csv
import sys


def splitmix64(seed: int):
    state = seed & 0xFFFFFFFFFFFFFFFF
    while True:
        state = (state + 0x9E3779B97F4A7C15) & 0xFFFFFFFFFFFFFFFF
        z = state
        z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9 & 0xFFFFFFFFFFFFFFFF
        z = (z ^ (z >> 27)) * 0x94D049BB133111EB & 0xFFFFFFFFFFFFFFFF
        yield z ^ (z >> 31)


def generate(count: int, dim: int, seed: int, out_dir: str = "."):
    rng = splitmix64(seed)

    def next_f():
        return ((next(rng) >> 32) & 0xFFFFFFFF) / 0xFFFFFFFF

    header = ["id"] + [f"d{i}" for i in range(dim)]
    with open(f"{out_dir}/vectors.csv", "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(header)
        for i in range(count):
            writer.writerow([i] + [next_f() for _ in range(dim)])

    print(f"generated {count} vectors of dim {dim} in {out_dir}/vectors.csv")


if __name__ == "__main__":
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 100_000
    dim = int(sys.argv[2]) if len(sys.argv) > 2 else 128
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 123
    out = sys.argv[4] if len(sys.argv) > 4 else "."
    generate(count, dim, seed, out)
