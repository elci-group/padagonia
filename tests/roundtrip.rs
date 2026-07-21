use padagonia::bench_support::generate_powerlaw;
use padagonia::provenance::Provenance;
use padagonia::store::Store;
use padagonia::Scalar;

#[test]
fn roundtrip_preserves_nodes_edges() {
    let mut original = Store::new();
    generate_powerlaw(&mut original, 1000, 5000, 7);
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let path = tmp.path();
    original.save(path).unwrap();

    let loaded = Store::load(path).unwrap();

    assert_eq!(original.nodes().len(), loaded.nodes().len());
    assert_eq!(original.edges().len(), loaded.edges().len());

    for (id, node) in original.nodes() {
        let other = loaded.nodes().get(id).expect("node missing");
        assert_eq!(node, other);
    }

    for (id, edge) in original.edges() {
        let other = loaded.edges().get(id).expect("edge missing");
        assert_eq!(edge, other);
    }

    assert_eq!(original.facts(), loaded.facts());
    assert_eq!(original.string_table(), loaded.string_table());
    assert_eq!(
        sorted_index(original.node_label_index()),
        sorted_index(loaded.node_label_index())
    );
    assert_eq!(
        sorted_index(original.edge_label_index()),
        sorted_index(loaded.edge_label_index())
    );
    assert_eq!(
        sorted_index(original.outgoing()),
        sorted_index(loaded.outgoing())
    );
    assert_eq!(
        sorted_index(original.incoming()),
        sorted_index(loaded.incoming())
    );
    assert_eq!(original.next_node_id(), loaded.next_node_id());
    assert_eq!(original.next_edge_id(), loaded.next_edge_id());
    assert_eq!(original.stats(), loaded.stats());
}

#[test]
fn roundtrip_rebuilds_id_counters_for_future_writes() {
    let mut original = Store::new();
    let a = original.add_node(
        "Person",
        vec![("name", Scalar::String("a".to_string()))],
        None,
        Provenance::new("agent", "model", 1.0, 0.0, 1, vec![]),
    );
    let b = original.add_node(
        "Person",
        vec![("name", Scalar::String("b".to_string()))],
        None,
        Provenance::new("agent", "model", 1.0, 0.0, 1, vec![]),
    );
    original.add_edge(
        a,
        b,
        "knows",
        vec![],
        None,
        Provenance::new("agent", "model", 1.0, 0.0, 1, vec![]),
    );

    let tmp = tempfile::NamedTempFile::new().unwrap();
    original.save(tmp.path()).unwrap();
    let mut loaded = Store::load(tmp.path()).unwrap();

    let next_node = loaded.add_node(
        "Person",
        vec![("name", Scalar::String("c".to_string()))],
        None,
        Provenance::new("agent", "model", 1.0, 0.0, 1, vec![]),
    );
    let next_edge = loaded.add_edge(
        b,
        next_node,
        "knows",
        vec![],
        None,
        Provenance::new("agent", "model", 1.0, 0.0, 1, vec![]),
    );

    assert_eq!(next_node.0, 2);
    assert_eq!(next_edge.0, 1);
}

fn sorted_index<K, V>(index: &ahash::AHashMap<K, Vec<V>>) -> Vec<(K, Vec<V>)>
where
    K: Copy + Ord,
    V: Copy + Ord,
{
    let mut entries: Vec<_> = index
        .iter()
        .map(|(key, values)| {
            let mut values = values.clone();
            values.sort();
            (*key, values)
        })
        .collect();
    entries.sort_by_key(|(key, _)| *key);
    entries
}
