use padagonia::bench_support::{generate_vectors, Rng};
use padagonia::hnsw::Distance;
use padagonia::ontology::StringTableExt;
use padagonia::provenance::Provenance;
use padagonia::query::QueryEngine;
use padagonia::store::Store;
use padagonia::{NodeId, Scalar};
use std::collections::HashSet;

#[test]
fn recall_on_random_data() {
    let mut store = Store::new();
    generate_vectors(&mut store, 1_000, 32, 42);
    let engine = QueryEngine::new(&store);

    let index = store.build_hnsw_index_with_seed(Distance::Euclidean, 16, 200, 10, 42);

    let mut rng = Rng::new(7);
    let mut total_recall = 0.0;
    let queries = 100;
    for _ in 0..queries {
        let query: Vec<f32> = (0..32).map(|_| rng.next_f32()).collect();
        let hnsw: HashSet<NodeId> = index
            .search(&query, 10, 10)
            .into_iter()
            .map(|(pid, _)| NodeId(pid.0))
            .collect();
        let brute: Vec<NodeId> = engine
            .brute_force_vector_search(&query, 10, None)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        let hits = brute.iter().filter(|id| hnsw.contains(id)).count();
        total_recall += hits as f64 / brute.len() as f64;
    }
    let avg_recall = total_recall / queries as f64;
    assert!(
        avg_recall >= 0.85,
        "average recall {:.3} below 0.85",
        avg_recall
    );
}

#[test]
fn exact_when_k_equals_n() {
    let mut store = Store::new();
    generate_vectors(&mut store, 50, 8, 11);
    let index = store.build_hnsw_index_with_seed(Distance::Euclidean, 16, 200, 50, 11);

    // Query using the first vector in the store.
    let query = store
        .nodes
        .values()
        .next()
        .and_then(|n| n.embedding.clone())
        .unwrap();
    let results = index.search(&query, 50, 50);
    assert_eq!(results.len(), 50);

    let returned: HashSet<NodeId> = results.into_iter().map(|(pid, _)| NodeId(pid.0)).collect();
    let all: HashSet<NodeId> = store.nodes.keys().copied().collect();
    assert_eq!(returned, all);
}

#[test]
fn label_filter_works() {
    let mut store = Store::new();
    let provenance = Provenance::new("a", "m", 1.0, 0.0, 1, vec![]);
    let mut rng = Rng::new(99);

    for i in 0..100 {
        let embedding: Vec<f32> = (0..16).map(|_| rng.next_f32()).collect();
        let label = if i % 2 == 0 { "A" } else { "B" };
        store.add_node(
            label,
            vec![("name", Scalar::String(format!("node_{}", i)))],
            Some(embedding),
            provenance.clone(),
        );
    }

    let engine = QueryEngine::new(&store);
    let label_a = store.string_table.label_id("A").unwrap();
    let query: Vec<f32> = (0..16).map(|_| rng.next_f32()).collect();
    let results = engine.vector_search(&query, 10, Some(label_a), 50);

    assert!(!results.is_empty());
    for (nid, _) in &results {
        let node = &store.nodes[nid];
        assert_eq!(node.label, label_a);
    }
}

#[test]
fn deterministic_with_seed() {
    let mut store = Store::new();
    generate_vectors(&mut store, 200, 16, 55);

    let index1 = store.build_hnsw_index_with_seed(Distance::Euclidean, 16, 200, 20, 123);
    let index2 = store.build_hnsw_index_with_seed(Distance::Euclidean, 16, 200, 20, 123);

    let query: Vec<f32> = (0..16).map(|i| i as f32 / 16.0).collect();
    let r1: Vec<NodeId> = index1
        .search(&query, 10, 20)
        .into_iter()
        .map(|(pid, _)| NodeId(pid.0))
        .collect();
    let r2: Vec<NodeId> = index2
        .search(&query, 10, 20)
        .into_iter()
        .map(|(pid, _)| NodeId(pid.0))
        .collect();
    assert_eq!(r1, r2);
}
