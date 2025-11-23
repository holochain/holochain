# Repository Guidelines

## Project Structure & Module Organization
- Core crates live in `crates/` (e.g. `holochain`, `holochain_types`, `holochain_state`). Each crate exposes a `src/` directory, unit tests in-line, and integration tests under `tests/` when needed.
- `scripts/` contains the supported task runners; `holonix/` and `nix/` directories are deprecated and may be ignored.
- Documentation resources are under `docs/` and complement the inline code documentation.
- When adding new features, group code by crate responsibility. For example, types into `holochain_types` and data logic into `holochain_state` and functionality into `holochain`. 
- Inline zomes are preferred in sweettests; add wasm artifacts only when absolutely required.

## Build, Test, and Development Commands
- `make static-all` - to run all static checks
- `cargo test -p <crate-name>` - run focused tests for a single crate while iterating locally.
- `make test-workspace-wasmer_sys` - to run all the tests from the root of the workspace.

## Coding Style & Naming Conventions
- Run `cargo fmt --all` before submitting; formatting follows upstream Rust defaults.
- Document public APIs with `///` rustdoc comments; prefer explicit `use` paths within crates.
- Feature flags and cargo package names mirror crate directories; avoid introducing new abbreviations without discussion.

## Testing Guidelines
- Tokio-based async tests use `#[tokio::test(flavor = "multi_thread")]`; match existing patterns.
- Avoid adding new `proptest` or fuzzing suites; this testing approach isn't currently part of the testing approach.
- Place integration tests under the crate’s `tests/` directory; name files `{feature}_tests.rs`; link new test modules to the `integration.rs` if present so that only one test binary is needed.
- Ensure regressions include targeted tests; prefer unit tests near the affected module and rely on inline zomes in sweettests unless a wasm file is strictly necessary.

## Commit & Pull Request Guidelines
- Follow Conventional Commits (e.g., `fix: guard missing bundle resources`); include concise bodies for context and wrap lines near 72 characters.
- Record all notable changes in `crates/holochain/CHANGELOG.md`, including bug fixes, new features, and breaking changes.
