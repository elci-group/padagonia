use padagonia::bench_support::generate_test_graph;
use padagonia::Store;

#[test]
fn repeated_save_atomically_replaces_the_previous_graph() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("graph.pad");

    let mut first = Store::new();
    generate_test_graph(&mut first, 5, 8, 1);
    first.save(&path).unwrap();

    let mut second = Store::new();
    generate_test_graph(&mut second, 12, 20, 2);
    second.save(&path).unwrap();

    let loaded = Store::load(&path).unwrap();
    assert_eq!(loaded.stats(), second.stats());

    let temporary_files: Vec<_> = std::fs::read_dir(directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(temporary_files.is_empty());
}

#[test]
fn failed_replacement_cleans_up_the_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("graph.pad");
    std::fs::create_dir(&destination).unwrap();

    let mut store = Store::new();
    generate_test_graph(&mut store, 3, 4, 3);
    assert!(store.save(&destination).is_err());
    assert!(destination.is_dir());

    let temporary_files: Vec<_> = std::fs::read_dir(directory.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
        .collect();
    assert!(temporary_files.is_empty());
}

#[test]
fn save_rejects_inconsistent_embedding_dimensions() {
    let directory = tempfile::tempdir().unwrap();
    let mut store = Store::new();
    let provenance = padagonia::Provenance::new("test", "model", 1.0, 0.0, 1, vec![]);
    store.add_node("A", vec![], Some(vec![0.0, 1.0]), provenance.clone());
    store.add_node("A", vec![], Some(vec![0.0, 1.0, 2.0]), provenance);

    assert!(matches!(
        store.save(directory.path().join("graph.pad")),
        Err(padagonia::StoreError::InvalidValue { .. })
    ));
}
