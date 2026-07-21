use padagonia::bench_support::generate_powerlaw;
use padagonia::store::Store;
use std::fs;
use std::path::PathBuf;

#[test]
fn load_and_load_seq_are_identical() {
    let mut original = Store::new();
    generate_powerlaw(&mut original, 5000, 25000, 11);
    let path = PathBuf::from("/tmp/padagonia_parallel.pad");
    original.save(&path).unwrap();

    let parallel = Store::load(&path).unwrap();
    let sequential = Store::load_seq(&path).unwrap();

    assert_eq!(parallel.nodes(), sequential.nodes());
    assert_eq!(parallel.edges(), sequential.edges());
    assert_eq!(parallel.next_node_id(), sequential.next_node_id());
    assert_eq!(parallel.next_edge_id(), sequential.next_edge_id());

    fs::remove_file(&path).ok();
}
