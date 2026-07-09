//! CLI argument parsing, help output, and command dispatch for the padagonia binary.

use crate::bench_support::{generate_powerlaw, generate_vectors, Rng};
use crate::hnsw::{Distance, DEFAULT_EF_CONSTRUCTION, DEFAULT_M};
use crate::ontology::StringTableExt;
use crate::projection::Projection;
use crate::query::QueryEngine;
use crate::store::Store;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ANSI style helpers for terminal output.
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const BLUE: &str = "\x1b[34m";
const RESET: &str = "\x1b[0m";

fn print_help() {
    let logo = r#"
             ╭──────────────╮
            ╱   ●       ●    ╲
           │    ╲  P  ╱      │
           │ ●───●───●───●   │
           │ │    ╲ ╱    │   │
           │ ●─────●─────●   │
            ╲   ╱     ╲   ╱
             ╰──────────────╯
"#;

    println!("{}", logo);
    println!(
        "{BOLD}{CYAN}PADAGONIA{RESET}{BOLD} — Parallelisation Accessible Database Advance Generative Ontology Networked Information Architecture{RESET}"
    );
    println!();
    println!(
        "{DIM}An ontology-native, immutable, provenance-rich graph store designed for\n\
         autonomous AI agents: nodes, edges, facts, embeddings, and HNSW vector search.{RESET}"
    );
    println!();
    println!("{BOLD}VERSION{RESET}    {}", VERSION.trim());
    println!("{BOLD}REPOSITORY{RESET} https://github.com/elci-group/padagonia");
    println!();
    println!("{BOLD}USAGE:{RESET}");
    println!("    {CYAN}padagonia{RESET} {BLUE}<COMMAND>{RESET} [OPTIONS]");
    println!();
    println!("{BOLD}COMMANDS:{RESET}");

    let commands: &[(&str, &str)] = &[
        (
            "ingest",
            "Generate a synthetic graph and save it to a PADAGONIA file",
        ),
        ("load", "Load a PADAGONIA file and print statistics"),
        ("bfs", "Run a breadth-first search from a starting node"),
        ("to-json", "Export a PADAGONIA file to JSON"),
        (
            "vector-search",
            "Search node embeddings with the HNSW approximate NN index",
        ),
        (
            "bench",
            "Run graph ingestion / save / load / traversal benchmarks",
        ),
        (
            "bench-vectors",
            "Run vector-search build / search / recall benchmarks",
        ),
        ("server", "Start the PADAGONIA HTTP server"),
        ("help", "Print this help message"),
    ];

    for (name, desc) in commands {
        println!("    {CYAN}{:<16}{RESET} {}", name, desc);
    }

    println!();
    println!("{BOLD}GLOBAL OPTIONS:{RESET}");
    println!("    {BLUE}-h{RESET}, {BLUE}--help{RESET}      Print this help message");
    println!("    {BLUE}-V{RESET}, {BLUE}--version{RESET}   Print version information");
    println!();
    println!("{BOLD}EXAMPLES:{RESET}");
    println!("    {DIM}# Ingest a synthetic graph{RESET}");
    println!(
        "    {CYAN}padagonia{RESET} ingest --nodes 10000 --edges 50000 --seed 1 --out graph.pad"
    );
    println!();
    println!("    {DIM}# BFS with optional relation filter{RESET}");
    println!(
        "    {CYAN}padagonia{RESET} bfs --in graph.pad --start 0 --depth 4 --relation works_for"
    );
    println!();
    println!("    {DIM}# Approximate nearest-neighbour search over embeddings{RESET}");
    println!(
        "    {CYAN}padagonia{RESET} vector-search --in graph.pad --k 10 --ef 200 --label Person"
    );
    println!();
    println!("    {DIM}# Start the HTTP server{RESET}");
    println!("    {CYAN}padagonia{RESET} server --config padagonia.toml");
    println!();
    println!("    {DIM}# Run the full benchmark suite{RESET}");
    println!("    {CYAN}padagonia{RESET} bench --nodes 100000 --edges 500000");
    println!("    {CYAN}padagonia{RESET} bench-vectors --nodes 50000 --dim 128 --k 10 --ef 200");
}

fn parse_flag<T: std::str::FromStr>(args: &[String], flag: &str) -> Option<T> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
}

fn parse_flag_str<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
}

pub fn run() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args[1] == "help" || args[1] == "-h" || args[1] == "--help" {
        print_help();
        std::process::exit(0);
    }

    if args[1] == "-V" || args[1] == "--version" {
        println!("padagonia {}", VERSION.trim());
        std::process::exit(0);
    }

    let cmd = args[1].as_str();
    let rest = &args[2..];

    metrics::counter!("padagonia_cli_commands_total", "command" => cmd.to_string()).increment(1);

    match cmd {
        "ingest" => cmd_ingest(rest),
        "load" => cmd_load(rest),
        "bfs" => cmd_bfs(rest),
        "to-json" => cmd_to_json(rest),
        "vector-search" => cmd_vector_search(rest),
        "bench" => cmd_bench(rest),
        "bench-vectors" => cmd_bench_vectors(rest),
        "server" => cmd_server(rest),
        _ => {
            eprintln!("{}error:{} unknown command '{}'\n", BOLD, RESET, cmd);
            print_help();
            std::process::exit(1);
        }
    }
}

fn cmd_ingest(args: &[String]) {
    let nodes: usize = parse_flag(args, "--nodes").expect("--nodes required");
    let edges: usize = parse_flag(args, "--edges").expect("--edges required");
    let seed: u64 = parse_flag(args, "--seed").expect("--seed required");
    let out: &str = parse_flag_str(args, "--out").expect("--out required");

    let mut store = Store::new();
    let t0 = Instant::now();
    generate_powerlaw(&mut store, nodes, edges, seed);
    let ingest_time = t0.elapsed();

    let t1 = Instant::now();
    store.save(out).expect("save failed");
    let save_time = t1.elapsed();

    let (n, e, f, l, r) = store.stats();
    println!(
        "ingested {} nodes, {} edges, {} facts in {:?} ({:.0} items/s)",
        n,
        e,
        f,
        ingest_time,
        (n + e) as f64 / ingest_time.as_secs_f64()
    );
    println!(
        "saved to {} in {:?} (labels={}, relations={})",
        out, save_time, l, r
    );
}

fn cmd_load(args: &[String]) {
    let path: &str = parse_flag_str(args, "--in").expect("--in required");
    let t0 = Instant::now();
    let store = Store::load(path).expect("load failed");
    let load_time = t0.elapsed();
    let (n, e, f, l, r) = store.stats();
    println!(
        "loaded {} nodes, {} edges, {} facts in {:?} ({:.0} items/s)",
        n,
        e,
        f,
        load_time,
        (n + e) as f64 / load_time.as_secs_f64()
    );
    println!("labels={}, relations={}", l, r);
}

fn cmd_bfs(args: &[String]) {
    let path: &str = parse_flag_str(args, "--in").expect("--in required");
    let start: u64 = parse_flag(args, "--start").expect("--start required");
    let depth: usize = parse_flag(args, "--depth").expect("--depth required");
    let relation: Option<String> = parse_flag_str(args, "--relation").map(|s| s.to_string());

    let store = Store::load(path).expect("load failed");
    let relation_id = relation
        .as_ref()
        .map(|r| store.string_table.relation_id(r).expect("unknown relation"));
    let engine = QueryEngine::new(&store);
    let start_id = crate::NodeId(start);
    let t0 = Instant::now();
    let result = engine.bfs(start_id, depth, relation_id, None);
    let bfs_time = t0.elapsed();
    println!(
        "BFS from {} depth {} reached {} nodes in {:?}",
        start,
        depth,
        result.len(),
        bfs_time
    );
}

fn cmd_to_json(args: &[String]) {
    let path: &str = parse_flag_str(args, "--in").expect("--in required");
    let out: &str = parse_flag_str(args, "--out").expect("--out required");
    let store = Store::load(path).expect("load failed");
    let json = store.to_json();
    fs::write(
        out,
        serde_json::to_string_pretty(&json).expect("json serialize"),
    )
    .expect("write failed");
    println!("wrote {}", out);
}

fn cmd_vector_search(args: &[String]) {
    let path: &str = parse_flag_str(args, "--in").expect("--in required");
    let k: usize = parse_flag(args, "--k").unwrap_or(10);
    let ef: usize = parse_flag(args, "--ef").unwrap_or(50);
    let metric: &str = parse_flag_str(args, "--metric").unwrap_or("euclidean");
    let label: Option<String> = parse_flag_str(args, "--label").map(|s| s.to_string());

    let distance = match metric {
        "cosine" => Distance::Cosine,
        _ => Distance::Euclidean,
    };

    let store = Store::load(path).expect("load failed");
    let label_id = label
        .as_ref()
        .map(|l| store.string_table.label_id(l).expect("unknown label"));

    // Deterministic query: first node that has an embedding.
    let query = store
        .nodes
        .values()
        .find_map(|n| n.embedding.clone())
        .unwrap_or_default();

    let t0 = Instant::now();
    let index = store.build_hnsw_index(distance, DEFAULT_M, DEFAULT_EF_CONSTRUCTION, ef.max(k));
    let build_time = t0.elapsed();

    let t1 = Instant::now();
    let mut results: Vec<(crate::NodeId, f32)> = index
        .search(&query, k, ef.max(k))
        .into_iter()
        .map(|(pid, dist)| (crate::NodeId(pid.0), dist))
        .collect();
    if let Some(lid) = label_id {
        results = results
            .into_iter()
            .filter_map(|(nid, dist)| {
                store.nodes.get(&nid).and_then(|n| {
                    if n.label == lid {
                        Some((nid, dist))
                    } else {
                        None
                    }
                })
            })
            .collect();
        results.truncate(k);
    }
    let search_time = t1.elapsed();

    println!(
        "built HNSW index in {:?} ({} points, {} dims)",
        build_time,
        index.len(),
        query.len()
    );
    println!("top-{} vector search in {:?}:", k, search_time);
    for (id, dist) in &results {
        println!("  {} {:.6}", id.0, dist);
    }
}

fn cmd_bench(args: &[String]) {
    let nodes: usize = parse_flag(args, "--nodes").unwrap_or(100_000);
    let edges: usize = parse_flag(args, "--edges").unwrap_or(500_000);
    let seed: u64 = parse_flag(args, "--seed").unwrap_or(42);

    let runs = 5;
    let mut ingest_times = Vec::with_capacity(runs);
    let mut save_times = Vec::with_capacity(runs);
    let mut load_times = Vec::with_capacity(runs);
    let mut load_seq_times = Vec::with_capacity(runs);
    let mut bfs_times = Vec::with_capacity(runs);
    let mut filter_times = Vec::with_capacity(runs);

    let tmp_pad = PathBuf::from("target/padagonia_bench.pad");
    fs::create_dir_all("target").ok();

    // Generate once.
    let mut base_store = Store::new();
    generate_powerlaw(&mut base_store, nodes, edges, seed);
    let (node_count, edge_count, fact_count, label_count, relation_count) = base_store.stats();

    for _ in 0..runs {
        let mut store = Store::new();
        let t0 = Instant::now();
        generate_powerlaw(&mut store, nodes, edges, seed);
        ingest_times.push(t0.elapsed().as_secs_f64());

        let t1 = Instant::now();
        store.save(&tmp_pad).unwrap();
        save_times.push(t1.elapsed().as_secs_f64());

        let t2 = Instant::now();
        let loaded = Store::load(&tmp_pad).unwrap();
        load_times.push(t2.elapsed().as_secs_f64());

        let t3 = Instant::now();
        let _loaded_seq = Store::load_seq(&tmp_pad).unwrap();
        load_seq_times.push(t3.elapsed().as_secs_f64());

        let engine = QueryEngine::new(&loaded);
        let start = loaded
            .outgoing
            .keys()
            .next()
            .copied()
            .unwrap_or(crate::NodeId(0));
        let t4 = Instant::now();
        let _ = engine.bfs(start, 4, None, None);
        bfs_times.push(t4.elapsed().as_secs_f64());

        let works_for = loaded.string_table.relation_id("works_for").unwrap();
        let t5 = Instant::now();
        let _ = engine.by_relation(works_for);
        filter_times.push(t5.elapsed().as_secs_f64());
    }

    let median = |v: &mut [f64]| -> f64 {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };

    let summary = serde_json::json!({
        "nodes": node_count,
        "edges": edge_count,
        "facts": fact_count,
        "labels": label_count,
        "relations": relation_count,
        "ingest_seconds": median(&mut ingest_times),
        "save_seconds": median(&mut save_times),
        "load_seconds": median(&mut load_times),
        "load_seq_seconds": median(&mut load_seq_times),
        "bfs_seconds": median(&mut bfs_times),
        "filter_by_relation_seconds": median(&mut filter_times),
        "ingest_throughput_items_per_sec": (node_count + edge_count) as f64 / median(&mut ingest_times),
        "load_throughput_items_per_sec": (node_count + edge_count) as f64 / median(&mut load_times),
        "file_bytes": fs::metadata(&tmp_pad).map(|m| m.len()).unwrap_or(0),
    });

    let out = PathBuf::from("target/padagonia_bench_summary.json");
    fs::write(&out, serde_json::to_string_pretty(&summary).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    println!("wrote {}", out.display());
}

fn cmd_bench_vectors(args: &[String]) {
    let nodes: usize = parse_flag(args, "--nodes").unwrap_or(50_000);
    let dim: usize = parse_flag(args, "--dim").unwrap_or(128);
    let k: usize = parse_flag(args, "--k").unwrap_or(10);
    let ef: usize = parse_flag(args, "--ef").unwrap_or(200);
    let m: usize = parse_flag(args, "--m").unwrap_or(DEFAULT_M);
    let ef_construction: usize =
        parse_flag(args, "--ef-construction").unwrap_or(DEFAULT_EF_CONSTRUCTION);
    let seed: u64 = parse_flag(args, "--seed").unwrap_or(123);
    let queries: usize = parse_flag(args, "--queries").unwrap_or(50);

    let mut store = Store::new();
    generate_vectors(&mut store, nodes, dim, seed);
    let engine = QueryEngine::new(&store);

    let mut rng = Rng::new(7);
    let query_vecs: Vec<Vec<f32>> = (0..queries)
        .map(|_| (0..dim).map(|_| rng.next_f32()).collect())
        .collect();

    let t0 = Instant::now();
    let index = store.build_hnsw_index(Distance::Euclidean, m, ef_construction, ef.max(k));
    let build_time = t0.elapsed().as_secs_f64();

    let t1 = Instant::now();
    let mut hnsw_results: Vec<Vec<crate::NodeId>> = Vec::with_capacity(queries);
    for q in &query_vecs {
        let r = index.search(q, k, ef.max(k));
        hnsw_results.push(r.into_iter().map(|(pid, _)| crate::NodeId(pid.0)).collect());
    }
    let search_time = t1.elapsed().as_secs_f64();

    // Brute force for recall and baseline timing.
    let t2 = Instant::now();
    let mut brute_results: Vec<Vec<crate::NodeId>> = Vec::with_capacity(queries);
    for q in &query_vecs {
        let r = engine.brute_force_vector_search(q, k, None);
        brute_results.push(r.into_iter().map(|(id, _)| id).collect());
    }
    let brute_time = t2.elapsed().as_secs_f64();

    let mut recall_sum = 0.0;
    for (h, b) in hnsw_results.iter().zip(brute_results.iter()) {
        let hset: HashSet<_> = h.iter().copied().collect();
        let hits = b.iter().filter(|id| hset.contains(id)).count();
        recall_sum += hits as f64 / k as f64;
    }
    let recall = recall_sum / queries as f64;

    let summary = serde_json::json!({
        "vectors": nodes,
        "dim": dim,
        "k": k,
        "ef": ef,
        "build_seconds": build_time,
        "search_seconds": search_time / queries as f64,
        "brute_force_avg_seconds": brute_time / queries as f64,
        "recall": recall,
    });

    let out = PathBuf::from("target/padagonia_hnsw_summary.json");
    fs::create_dir_all("target").ok();
    fs::write(&out, serde_json::to_string_pretty(&summary).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    println!("wrote {}", out.display());
}

fn cmd_server(args: &[String]) {
    let config_path: &str = parse_flag_str(args, "--config").unwrap_or("padagonia.toml");
    let settings =
        crate::app_config::Settings::load_from(config_path).expect("failed to load configuration");

    let rt = tokio::runtime::Runtime::new().expect("failed to create Tokio runtime");
    rt.block_on(crate::server::serve(settings))
        .expect("server failed");
}
