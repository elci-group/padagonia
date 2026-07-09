# Contributing to PADAGONIA

Thank you for your interest in PADAGONIA! This document explains how to
contribute effectively.

## Code of Conduct

All contributors are expected to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## How to Contribute

- **Bug reports:** Open an issue with a minimal reproduction and the version of
  PADAGONIA you are using.
- **Feature requests:** Open an issue describing the use case and the proposed
  API or behavior.
- **Pull requests:** We welcome fixes, documentation improvements, and small,
  focused features. For large changes, please open an issue first to discuss the
  design.

## Development Workflow

1. Fork the repository and create a feature branch.
2. Make your changes, including tests where appropriate.
3. Run the full Rust checks:

   ```bash
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   cargo bench
   ```

4. Update `CHANGELOG.md` with a short entry describing your change.
5. Commit with a clear message and open a pull request.

## Coding Conventions

- Follow the existing Rust style (`cargo fmt`).
- Keep public APIs documented.
- Prefer small, focused commits and pull requests.
- Add tests for new functionality and bug fixes.

## Licensing

By contributing to PADAGONIA, you agree that your contributions will be
licensed under the MIT OR Apache-2.0 dual license, matching the rest of the
project.
