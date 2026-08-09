use padagonia::{NamespaceId, Provenance, Store};

fn provenance() -> Provenance {
    Provenance::new("test", "fixture", 1.0, 0.0, 1, vec![])
}

#[test]
fn namespaced_nodes_and_edges_are_accepted() {
    let mut store = Store::new();
    let namespace = NamespaceId::new("workspace-a").unwrap();
    let left = store
        .add_node_in_namespace(
            namespace.clone(),
            "run-1",
            "Run",
            vec![],
            None,
            provenance(),
        )
        .unwrap();
    let right = store
        .add_node_in_namespace(
            namespace.clone(),
            "opportunity-1",
            "Opportunity",
            vec![],
            None,
            provenance(),
        )
        .unwrap();

    store
        .add_edge_in_namespace(
            namespace,
            "supports-1",
            left,
            right,
            "supports",
            vec![],
            None,
            provenance(),
        )
        .unwrap();
    assert_eq!(store.edges().len(), 1);
}

#[test]
fn cross_namespace_edges_are_rejected_before_mutation() {
    let mut store = Store::new();
    let first = NamespaceId::new("workspace-a").unwrap();
    let second = NamespaceId::new("workspace-b").unwrap();
    let left = store
        .add_node_in_namespace(first.clone(), "left", "Run", vec![], None, provenance())
        .unwrap();
    let right = store
        .add_node_in_namespace(second, "right", "Run", vec![], None, provenance())
        .unwrap();

    assert!(store
        .add_edge_in_namespace(
            first,
            "crossing",
            left,
            right,
            "supports",
            vec![],
            None,
            provenance()
        )
        .is_err());
    assert!(store.edges().is_empty());
}
