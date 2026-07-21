use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use padagonia::bench_support::{generate_powerlaw, generate_vectors, Rng};
use padagonia::hnsw::{Distance, DEFAULT_EF_CONSTRUCTION, DEFAULT_M};
use padagonia::ontology::StringTableExt;
use padagonia::query::QueryEngine;
use padagonia::store::Store;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn make_tmp() -> PathBuf {
    PathBuf::from("target/bench.pad")
}

fn bench_ingest(c: &mut Criterion) {
    let mut group = c.benchmark_group("ingest");
    for (nodes, edges) in [(10_000usize, 50_000usize), (100_000, 500_000)] {
        group.throughput(Throughput::Elements((nodes + edges) as u64));
        group.bench_with_input(
            format!("{}_{}", nodes, edges),
            &(nodes, edges),
            |b, &(n, e)| {
                b.iter(|| {
                    let mut store = Store::new();
                    generate_powerlaw(&mut store, n, e, 99);
                    black_box(store);
                });
            },
        );
    }
    group.finish();
}

fn bench_save(c: &mut Criterion) {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 100_000, 500_000, 99);
    let tmp = make_tmp();
    let mut group = c.benchmark_group("save");
    group.throughput(Throughput::Elements(
        (store.nodes().len() + store.edges().len()) as u64,
    ));
    group.bench_function("100k_500k", |b| {
        b.iter(|| {
            store.save(&tmp).unwrap();
        });
    });
    group.finish();
}

fn bench_load(c: &mut Criterion) {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 100_000, 500_000, 99);
    let tmp = make_tmp();
    store.save(&tmp).unwrap();
    let bytes = fs::metadata(&tmp).unwrap().len();

    let mut group = c.benchmark_group("load");
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("parallel_100k_500k", |b| {
        b.iter(|| {
            let loaded = Store::load(&tmp).unwrap();
            black_box(loaded);
        });
    });
    group.bench_function("sequential_100k_500k", |b| {
        b.iter(|| {
            let loaded = Store::load_seq(&tmp).unwrap();
            black_box(loaded);
        });
    });
    group.finish();
}

fn bench_bfs(c: &mut Criterion) {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 100_000, 500_000, 99);
    let engine = QueryEngine::new(&store);
    let start = store
        .outgoing()
        .keys()
        .next()
        .copied()
        .unwrap_or(padagonia::NodeId(0));

    let mut group = c.benchmark_group("bfs");
    group.bench_function("depth4_100k_500k", |b| {
        b.iter(|| {
            let result = engine.bfs(start, 4, None, None);
            black_box(result);
        });
    });
    group.finish();
}

fn bench_filter(c: &mut Criterion) {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 100_000, 500_000, 99);
    let engine = QueryEngine::new(&store);
    let works_for = store.string_table().relation_id("works_for").unwrap();

    let mut group = c.benchmark_group("filter");
    group.bench_function("by_relation_100k_500k", |b| {
        b.iter(|| {
            let result = engine.by_relation(works_for);
            black_box(result);
        });
    });
    group.finish();
}

fn bench_hnsw(c: &mut Criterion) {
    const N: usize = 50_000;
    const DIM: usize = 128;
    const K: usize = 10;
    const EF: usize = 200;
    const M: usize = DEFAULT_M;
    const EF_CONSTRUCTION: usize = DEFAULT_EF_CONSTRUCTION;

    let mut store = Store::new();
    generate_vectors(&mut store, N, DIM, 123);
    let engine = QueryEngine::new(&store);

    let mut rng = Rng::new(7);
    let queries: Vec<Vec<f32>> = (0..100)
        .map(|_| (0..DIM).map(|_| rng.next_f32()).collect())
        .collect();

    // --- build benchmark ---
    let mut build_group = c.benchmark_group("hnsw_build");
    build_group.sample_size(10);
    build_group.bench_function("50k_128d", |b| {
        b.iter(|| {
            let idx = store.build_hnsw_index(Distance::Euclidean, M, EF_CONSTRUCTION, EF);
            black_box(idx);
        });
    });
    build_group.finish();

    // Pre-build the index once for search/recall measurement.
    let index = store.build_hnsw_index(Distance::Euclidean, M, EF_CONSTRUCTION, EF);

    // --- brute-force baseline (once, for recall and timing comparison) ---
    let mut brute_times = Vec::with_capacity(queries.len());
    let mut brute_results: Vec<Vec<padagonia::NodeId>> = Vec::with_capacity(queries.len());
    for q in &queries {
        let t0 = Instant::now();
        let r = engine.brute_force_vector_search(q, K, None);
        brute_times.push(t0.elapsed().as_secs_f64());
        brute_results.push(r.into_iter().map(|(id, _)| id).collect());
    }
    let brute_avg = brute_times.iter().sum::<f64>() / brute_times.len() as f64;

    // --- search benchmark ---
    let mut search_group = c.benchmark_group("hnsw_search");
    search_group.sample_size(10);
    search_group.bench_function("50k_128d_k10", |b| {
        b.iter(|| {
            let mut all = Vec::with_capacity(queries.len());
            for q in &queries {
                let r = index.search(q, K, EF);
                all.push(r);
            }
            black_box(all);
        });
    });
    search_group.finish();

    // --- recall ---
    let mut recall_sum = 0.0;
    for (i, q) in queries.iter().enumerate() {
        let hnsw = index.search(q, K, EF);
        let h_ids: HashSet<padagonia::NodeId> = hnsw
            .iter()
            .map(|(pid, _)| padagonia::NodeId(pid.0))
            .collect();
        let hits = brute_results[i]
            .iter()
            .filter(|id| h_ids.contains(id))
            .count();
        recall_sum += hits as f64 / K as f64;
    }
    let recall = recall_sum / queries.len() as f64;

    // --- brute-force search benchmark ---
    let mut bf_group = c.benchmark_group("brute_force_search");
    bf_group.sample_size(10);
    bf_group.bench_function("50k_128d_k10", |b| {
        b.iter(|| {
            let mut all = Vec::with_capacity(queries.len());
            for q in &queries {
                let r = engine.brute_force_vector_search(q, K, None);
                all.push(r);
            }
            black_box(all);
        });
    });
    bf_group.finish();

    println!("\n=== HNSW vector search summary ===");
    println!("recall@{}: {:.3}", K, recall);
    println!("brute-force avg query: {:.3} ms", brute_avg * 1000.0);

    let summary = serde_json::json!({
        "vectors": N,
        "dim": DIM,
        "k": K,
        "ef": EF,
        "recall": recall,
        "brute_force_avg_seconds": brute_avg,
    });
    fs::create_dir_all("target").ok();
    fs::write(
        "target/padagonia_hnsw_summary.json",
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();
}

criterion_group!(
    benches,
    bench_ingest,
    bench_save,
    bench_load,
    bench_bfs,
    bench_filter,
    bench_hnsw
);
criterion_main!(benches);
