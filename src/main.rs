//! PADAGONIA command-line entry point.
//!
//! The CLI owns configuration loading, logging setup, and dispatch. Keeping
//! this binary thin makes the server and persistence layers testable as a
//! library and keeps external clients off internal storage maps.

fn main() {
    padagonia::cli::run();
}
