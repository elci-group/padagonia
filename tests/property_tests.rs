//! Property-based tests for PADAGONIA storage and operations.

use padagonia::bench_support::generate_powerlaw;
use padagonia::provenance::Provenance;
use padagonia::store::Store;
use padagonia::value::Scalar;
use proptest::prelude::*;

/// Strategy for generating scalar values
fn scalar_strategy() -> impl Strategy<Value = Scalar> {
    prop_oneof![
        Just(Scalar::Null),
        (0i64..1000).prop_map(Scalar::I64),
        (0.0f64..1000.0).prop_map(Scalar::F64),
        ".*{1,10}".prop_map(|s: String| Scalar::String(s)),
        any::<bool>().prop_map(Scalar::Bool),
    ]
}

/// Strategy for generating property maps
fn props_strategy() -> impl Strategy<Value = Vec<(String, Scalar)>> {
    prop::collection::vec(
        (".*{1,10}", scalar_strategy()),
        0..5, // 0-5 properties
    )
}

/// Strategy for generating provenance
fn provenance_strategy() -> impl Strategy<Value = Provenance> {
    (
        ".*{1,20}",                              // agent
        ".*{1,20}",                              // model
        0.0f32..1.0,                             // confidence
        0.0f32..1000.0,                          // cost
        any::<u64>(),                            // timestamp
        prop::collection::vec(".*{1,50}", 0..3), // evidence
    )
        .prop_map(|(agent, model, confidence, cost, timestamp, evidence)| {
            Provenance::new(agent, model, confidence, cost, timestamp, evidence)
        })
}

proptest! {
    #[test]
    fn store_save_load_roundtrip_preserves_all_data(seed in 0u64..1000) {
        let mut original_store = Store::new();
        generate_powerlaw(&mut original_store, 10, 20, seed);

        let original_nodes = original_store.nodes().clone();
        let original_edges = original_store.edges().clone();
        let original_facts = original_store.facts().clone();

        // Save to temp file
        let tmp = tempfile::NamedTempFile::new().unwrap();
        original_store.save(tmp.path()).unwrap();

        // Load from file
        let loaded_store = Store::load(tmp.path()).unwrap();

        // Verify all data is preserved
        assert_eq!(loaded_store.nodes().len(), original_nodes.len());
        assert_eq!(loaded_store.edges().len(), original_edges.len());
        assert_eq!(loaded_store.facts().len(), original_facts.len());

        // Verify node contents
        for (id, node) in &original_nodes {
            let loaded_node = loaded_store.nodes().get(id).unwrap();
            assert_eq!(loaded_node.label, node.label);
            assert_eq!(loaded_node.properties.len(), node.properties.len());
        }

        // Verify edge contents
        for (id, edge) in &original_edges {
            let loaded_edge = loaded_store.edges().get(id).unwrap();
            assert_eq!(loaded_edge.src, edge.src);
            assert_eq!(loaded_edge.dst, edge.dst);
            assert_eq!(loaded_edge.label, edge.label);
            assert_eq!(loaded_edge.properties.len(), edge.properties.len());
        }
    }

    #[test]
    fn add_node_increments_id_counter(props in props_strategy()) {
        let mut store = Store::new();
        let initial_count = store.nodes().len();

        let props_borrowed: Vec<(&str, Scalar)> = props
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();
        let _node_id = store.add_node("test_label", props_borrowed, None, Provenance::new("agent", "model", 0.5, 0.0, 0, vec![]));

        assert_eq!(store.nodes().len(), initial_count + 1);
    }

    #[test]
    fn add_edge_increments_id_counter(props in props_strategy()) {
        let mut store = Store::new();

        // Ensure source and destination nodes exist
        let src_id = store.add_node("src_label", vec![], None, Provenance::new("agent", "model", 0.5, 0.0, 0, vec![]));
        let dst_id = store.add_node("dst_label", vec![], None, Provenance::new("agent", "model", 0.5, 0.0, 0, vec![]));

        let initial_count = store.edges().len();

        let props_borrowed: Vec<(&str, Scalar)> = props
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();
        let _edge_id = store.add_edge(src_id, dst_id, "test_relation", props_borrowed, None, Provenance::new("agent", "model", 0.5, 0.0, 0, vec![]));

        assert_eq!(store.edges().len(), initial_count + 1);
    }

    #[test]
    fn string_table_registration_is_idempotent(name in ".*{1,50}") {
        let mut store = Store::new();

        let id1 = store.intern_label(&name);
        let id2 = store.intern_label(&name);

        assert_eq!(id1, id2);
    }

    #[test]
    fn different_strings_get_different_ids(name1 in ".*{1,50}", name2 in ".*{1,50}") {
        let mut store = Store::new();

        let id1 = store.intern_label(&name1);
        let id2 = store.intern_label(&name2);

        if name1 != name2 {
            assert_ne!(id1, id2);
        } else {
            assert_eq!(id1, id2);
        }
    }

    #[test]
    fn provenance_properties_are_preserved(prov in provenance_strategy()) {
        let mut store = Store::new();

        let node_id = store.add_node("test_label", vec![], None, prov.clone());
        let node = store.nodes().get(&node_id).unwrap();

        assert_eq!(node.provenance.agent, prov.agent);
        assert_eq!(node.provenance.model, prov.model);
        assert_eq!(node.provenance.confidence, prov.confidence);
        assert_eq!(node.provenance.cost, prov.cost);
        assert_eq!(node.provenance.timestamp, prov.timestamp);
        assert_eq!(node.provenance.evidence, prov.evidence);
    }
}
