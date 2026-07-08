use padagonia::bench_support::generate_powerlaw;
use padagonia::store::Store;
use std::fs;
use std::path::PathBuf;

#[test]
fn roundtrip_preserves_nodes_edges() {
    let mut original = Store::new();
    generate_powerlaw(&mut original, 1000, 5000, 7);
    let path = PathBuf::from("/tmp/padagonia_roundtrip.pad");
    original.save(&path).unwrap();

    let loaded = Store::load(&path).unwrap();

    assert_eq!(original.nodes.len(), loaded.nodes.len());
    assert_eq!(original.edges.len(), loaded.edges.len());

    for (id, node) in &original.nodes {
        let other = loaded.nodes.get(id).expect("node missing");
        assert_eq!(node, other);
    }

    for (id, edge) in &original.edges {
        let other = loaded.edges.get(id).expect("edge missing");
        assert_eq!(edge, other);
    }

    fs::remove_file(&path).ok();
}
