use padagonia::bench_support::generate_powerlaw;
use padagonia::fact::FactSubject;
use padagonia::ontology::StringTableExt;
use padagonia::provenance::Provenance;
use padagonia::query::QueryEngine;
use padagonia::store::Store;

#[test]
fn bfs_distances_are_correct() {
    let mut store = Store::new();
    let a = store.add_node(
        "A",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    let b = store.add_node(
        "B",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    let c = store.add_node(
        "C",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    let d = store.add_node(
        "D",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );

    store.add_edge(
        a,
        b,
        "r",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    store.add_edge(
        b,
        c,
        "r",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    store.add_edge(
        a,
        d,
        "r",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );

    let engine = QueryEngine::new(&store);
    let reached: std::collections::HashMap<_, _> =
        engine.bfs(a, 10, None, None).into_iter().collect();

    assert_eq!(reached[&a], 0);
    assert_eq!(reached[&b], 1);
    assert_eq!(reached[&d], 1);
    assert_eq!(reached[&c], 2);
}

#[test]
fn relation_filter_limits_edges() {
    let mut store = Store::new();
    let a = store.add_node(
        "A",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    let b = store.add_node(
        "B",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    let c = store.add_node(
        "C",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );

    store.add_edge(
        a,
        b,
        "knows",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    store.add_edge(
        a,
        c,
        "works_for",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );

    let engine = QueryEngine::new(&store);
    let knows_id = store.string_table.relation_id("knows").unwrap();
    let outgoing_knows = engine.outgoing(a, Some(knows_id));
    assert_eq!(outgoing_knows.len(), 1);
    assert_eq!(outgoing_knows[0].dst, b);
}

#[test]
fn confidence_filter_skips_weak_edges() {
    let mut store = Store::new();
    let a = store.add_node(
        "A",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    let b = store.add_node(
        "B",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );
    let c = store.add_node(
        "C",
        vec![],
        None,
        Provenance::new("a", "m", 1.0, 0.0, 1, vec![]),
    );

    store.add_edge(
        a,
        b,
        "r",
        vec![],
        None,
        Provenance::new("a", "m", 0.9, 0.0, 1, vec![]),
    );
    store.add_edge(
        a,
        c,
        "r",
        vec![],
        None,
        Provenance::new("a", "m", 0.3, 0.0, 1, vec![]),
    );

    let engine = QueryEngine::new(&store);
    let reached = engine.bfs(a, 10, None, Some(0.5));
    assert!(reached.iter().any(|(n, _)| *n == b));
    assert!(!reached.iter().any(|(n, _)| *n == c));
}

#[test]
fn by_label_counts_match() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 1000, 5000, 13);
    let engine = QueryEngine::new(&store);
    let person_id = store.string_table.label_id("Person").unwrap();
    let by_person = engine.by_label(person_id);
    let brute_force: Vec<_> = store
        .nodes
        .values()
        .filter(|n| n.label == person_id)
        .collect();
    assert_eq!(by_person.len(), brute_force.len());
}

#[test]
fn facts_count_after_additions() {
    let mut store = Store::new();
    // Initial provenance has lower confidence so the added facts are the winners.
    let a = store.add_node(
        "A",
        vec![],
        None,
        Provenance::new("a", "m", 0.5, 0.0, 1, vec![]),
    );
    store.add_fact(
        FactSubject::Node(a),
        Provenance::new("a", "m", 0.9, 0.0, 2, vec![]),
    );
    store.add_fact(
        FactSubject::Node(a),
        Provenance::new("a", "m", 0.7, 0.0, 3, vec![]),
    );

    let engine = QueryEngine::new(&store);
    // add_node records the initial provenance as a fact, plus the two add_fact calls.
    assert_eq!(engine.facts_about(FactSubject::Node(a)).len(), 3);
    assert_eq!(
        engine
            .highest_confidence_fact(FactSubject::Node(a))
            .unwrap()
            .confidence,
        0.9
    );
}
