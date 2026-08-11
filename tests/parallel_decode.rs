use padagonia::bench_support::generate_powerlaw;
use padagonia::store::Store;

#[test]
fn load_and_load_seq_are_identical() {
    let mut original = Store::new();
    generate_powerlaw(&mut original, 5000, 25000, 11);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("parallel.pad");
    original.save(&path).unwrap();

    let parallel = Store::load(&path).unwrap();
    let sequential = Store::load_seq(&path).unwrap();

    assert_eq!(parallel.nodes(), sequential.nodes());
    assert_eq!(parallel.edges(), sequential.edges());
    assert_eq!(parallel.next_node_id(), sequential.next_node_id());
    assert_eq!(parallel.next_edge_id(), sequential.next_edge_id());
}
