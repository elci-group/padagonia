//! Golden binary fixtures for storage format testing.

use padagonia::bench_support::generate_powerlaw;
use padagonia::store::Store;
use std::fs;
use std::path::PathBuf;

/// Path to golden fixtures directory.
fn fixtures_dir() -> PathBuf {
    PathBuf::from("tests/fixtures")
}

/// Generate a golden fixture file with a simple test store.
pub fn generate_simple_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 5, 10, 2);

    let fixtures_path = fixtures_dir();
    fs::create_dir_all(&fixtures_path)?;

    let fixture_path = fixtures_path.join("simple_v2.padagonia");
    store.save(&fixture_path)?;

    println!("Generated golden fixture: {}", fixture_path.display());
    Ok(())
}

/// Load a golden fixture for testing.
pub fn load_simple_fixture() -> Result<Store, Box<dyn std::error::Error>> {
    let fixture_path = fixtures_dir().join("simple_v2.padagonia");
    Store::load(&fixture_path).map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_fixture_exists_and_loads() {
        // This test will fail if the fixture doesn't exist
        // Run `cargo test -- --ignored` to generate fixtures
        let result = load_simple_fixture();
        if result.is_err() {
            panic!("Golden fixture not found. Run `cargo test generate_golden_fixtures -- --ignored` to generate it.");
        }

        let store = result.unwrap();
        // Verify the fixture has expected content
        assert!(!store.nodes().is_empty(), "Fixture should contain nodes");
        assert!(!store.edges().is_empty(), "Fixture should contain edges");
    }
}

#[ignore]
#[test]
fn generate_golden_fixtures() {
    // Run this test to generate golden fixtures: cargo test generate_golden_fixtures -- --ignored
    generate_simple_fixture().expect("Failed to generate golden fixture");
}
