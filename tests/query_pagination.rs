use padagonia::{NamespaceId, Provenance, QueryEngine, Store, StringTableExt};

fn provenance() -> Provenance {
    Provenance::new("test", "fixture", 1.0, 0.0, 1, vec![])
}

#[test]
fn exact_lookup_and_cursor_pagination_are_namespace_scoped() {
    let mut store = Store::new();
    let namespace = NamespaceId::new("workspace-a").unwrap();
    let other = NamespaceId::new("workspace-b").unwrap();
    for index in 0..3 {
        store
            .add_node_in_namespace(
                namespace.clone(),
                format!("run-{index}"),
                "Run",
                vec![],
                None,
                provenance(),
            )
            .unwrap();
    }
    store
        .add_node_in_namespace(
            other.clone(),
            "run-other",
            "Run",
            vec![],
            None,
            provenance(),
        )
        .unwrap();

    let label = store.string_table().label_id("Run").unwrap();
    let engine = QueryEngine::new(&store);
    assert_eq!(
        engine.exact_node(&namespace, "run-1").unwrap().external_id,
        "run-1"
    );
    assert!(engine.exact_node(&namespace, "run-other").is_none());

    let first = engine.nodes_page(&padagonia::query::NodeQuery {
        namespace: Some(namespace.clone()),
        label: Some(label),
        limit: 2,
        ..Default::default()
    });
    assert_eq!(first.entries.len(), 2);
    let second = engine.nodes_page(&padagonia::query::NodeQuery {
        namespace: Some(namespace),
        label: Some(label),
        limit: 2,
        cursor: first.next_cursor,
        ..Default::default()
    });
    assert_eq!(second.entries.len(), 1);
    assert_ne!(first.entries[0].id, second.entries[0].id);
}
