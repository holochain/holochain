# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, lint, and test

- `make static-all` — full static check suite (fmt, toml, clippy, clippy-unstable, doc).
- `make build-workspace-wasmer-sys-cranelift` — builds all targets (including the `hc`/`hc_sandbox` binaries). Run this **before** `test-workspace-wasmer-sys-cranelift` if `target/debug/hc` doesn't already exist: the `hc_client`/`hc_sandbox` integration tests shell out to that compiled binary at a fixed path rather than building it themselves, and fail with "you need to build the workspace so the following file exists" if it's missing — a pure build-order gap, not a test failure. A fresh worktree/checkout that hasn't built `hc` yet will hit this on the first test run.
- `make test-workspace-wasmer-sys-cranelift` — full workspace test run from the repo root (uses `cargo nextest`, default features + iroh transport + cranelift).
- `cargo test -p <crate-name>` — focused tests for a single crate while iterating.
- `cargo nextest run -p <crate-name> <test_name>` — run a single test (nextest is the configured runner; see `.config/nextest.toml`).
- `cargo fmt --all` — format before submitting.
- `scripts/format-toml.sh` - format TOML files, if any have changed, before submitting.
- Toolchain is pinned in `rust-toolchain.toml` (currently 1.96.1) — do not bump without discussion.

Feature flags worth knowing (defined in the `Makefile`):
- `DEFAULT_FEATURES` = `transport-iroh,slow_tests,build_wasms,encryption`.
- Wasmer backends are tested separately: `wasmer-sys-cranelift` (default), `wasmer-sys-llvm`, or `wasmer-wasmi`. At least one must be enabled or the crate fails to compile.
- `UNSTABLE_FEATURES` adds `unstable-sharding,unstable-functions,unstable-migration` — use the `*-unstable` Make targets to exercise them.

## Architecture

This is a Cargo workspace; everything ships as crates under `crates/`. The big-picture layering, from the bottom up:

- **Hashing & primitives** — `holo_hash`, `timestamp`, `holochain_nonce`, `holochain_secure_primitive`, `holochain_util`.
- **Types** — `holochain_integrity_types` (types available to integrity zomes; minimal, deterministic), `holochain_zome_types` (re-exports + coordinator-zome types), `holochain_types` (host-side rich types built on the above).
- **Persistence** — `holochain_data` owns primitive SQLx data access and connection setup. `holochain_state` layers typed store APIs and workflow-facing operations on top.
- **Networking** — `holochain_p2p` wraps `kitsune2` and exposes the gossip / publish / get / block APIs the conductor uses.
- **Cascade** — Currently, `holochain_cascade` is the "fetch from local DBs, then fall back to the network" layer used by zome calls and validation. See the section about the in-progress migration to `holochain_data`.
- **Conductor / runtime** — `holochain` is the top crate. It owns:
  - `src/conductor/` — the long-running process: cells, interfaces, app/admin APIs, the ribosome store, space/cell management, config.
  - `src/core/` — domain logic: workflows, queue consumers, ribosome (WASM host), sys-validate / app-validate.
  - `src/sweettest/` — in-process test harness for spinning up conductors with inline or WASM zomes.
- **SDKs** — `hdi` (integrity) and `hdk` (coordinator) are the developer-facing crates that compile to WASM; `hdk_derive` provides the macros.
- **CLI / tooling** — `hc`, `hc_bundle`, `hc_sandbox`, `hc_service_check`, `holochain_terminal`, `client`, `hc_client`. `mr_bundle` is the bundle (DNA/hApp) format.
- **Test wasms** — `crates/test_utils/wasm/wasm_workspace/` contains compiled-to-wasm test zomes; `TestWasm` enum in `crates/test_utils/wasm/src/lib.rs` is the registry. **Prefer inline zomes (`InlineZomeSet` / `SweetInlineZomes`) over adding new test wasms** — only add a WASM artifact when wasm-execution machinery is actually under test (per CONTRIBUTING.md).

Design references: `docs/design/state_model.md` and `docs/design/data_model.md` document the DHT/source-chain schema and the data types that live in it.

`scripts/` holds the supported task runners. `holonix/` and `nix/` directories are deprecated and may be ignored.

## Project conventions

- **Where new code goes**: types into `holochain_types`, persistence into `holochain_data` and `holochain_state`, runtime/orchestration into `holochain`. Don't shortcut by piling logic into the top-level crate.
- **Data-access naming (`holochain_state` / `holochain_cascade`)**: `get_*` reads only local state; `retrieve_*` may combine local and network lookups. The distinction is meaningful at the cascade — a cascade `get` stays local while a cascade `retrieve` can fall back to the network. At the network boundary a fetch is itself called a `get`, and the HDK bundles everything under `get` because how data is returned is transparent to the application.
- **Testing**:
  - Unit tests are placed inline or in a submodule next to the code under test.
  - Integration tests go under the crate's `tests/` directory, named `{feature}_tests.rs`. If `tests/integration.rs` exists, link new modules there so only one test binary builds. This saves time spent on linking.
  - Use `#[tokio::test]` by default; only switch to `#[tokio::test(flavor = "multi_thread")]` when the test genuinely needs it.
  - Do not introduce new `proptest` or fuzzing suites.
  - Test functions must not be prefixed with `test_` — the `#[test]` / `#[tokio::test]` attribute already marks them.
  - Test-support code exposed from library crates must be feature-gated so it never compiles into production builds. Read-only inspection queries (op counts, existence checks) use `#[cfg(any(test, feature = "inspection"))]`; test-only writes and fixture builders use `#[cfg(feature = "test_utils")]` (which also enables `inspection`).
- **Errors**: prefer `thiserror` for crate error types; `anyhow` is for application/binary code, not library APIs.
- **Compiler warnings are not OK** in shared code (CONTRIBUTING.md). Fix, surgically `#[allow(...)]`, or escalate — don't disable globally.
- **Public API docs**: `///` rustdoc on public items; module/crate docs should describe structure.
- **Commits**: Conventional Commits (`feat:`, `fix:`, `refactor:`, etc.), bodies wrapped near 72 chars. Record notable changes in `crates/holochain/CHANGELOG.md` (bug fixes, new features, breaking changes).
- **PRs**: branch off `develop`; changes are squash merged into `develop`; changes go from `develop` → `main` at release time and `main` should always be ignored for development.

## Project principles

### Offline friendly

It has not become an officially supported mode of use, but it is a long-standing goal that Holochain should function well offline.

Holochain does not know whether it has an internet connection, or how well connected it is to peers. It only learns what's working when it attempts requests.

When making code changes, don't assume the network is available. Locally available data should always be returned and the user should be able to install and uninstall apps, create and read data, or progress validation of data with any content that is already available locally.

### Workflows

Workflows always refer to the code under `crates/holochain/src/core/workflow`. The behavior of the workflows is described more specifically in the file `docs/design/state_model.md`.

At a higher level, the workflows are supposed to operate as atomically as possible:
- The genesis workflow `crates/holochain/src/core/workflow/genesis_workflow.rs`, runs when a new cell is instantiated and creates the genesis chain entries for the agent who created the cell.
- The initialize zomes workflow `crates/holochain/src/core/workflow/initialize_zomes_workflow.rs`, is to support an application-level hook per cell, by a coordinator providing an `init` function. No other zome calls may proceed until the `init` hook returns a successful result. Any data authored by the app is persisted and then a special marker entry for the hook completing is written to the source chain.
- The call zome workflow `crates/holochain/src/core/workflow/call_zome_workflow.rs`, executes a coordinator WASM call and captures created content into the in-memory scratch space. If there is any created content, then it is validated using inline validation. If the call fails, an error is returned, and if it succeeds then the newly authored data is written to the database in a transaction.
- The publishing workflow `crates/holochain/src/core/workflow/publish_dht_ops_workflow.rs`, is the quick path to share newly authored data with other peers. This is in contrast with Kitsune2's gossip which can be slower to share content in the background. The publish workflow also acts as a notification system to request validation receipts from peers.
- The incoming DHT ops workflow `crates/holochain/src/core/workflow/incoming_dht_ops_workflow.rs`, is the workflow that receives content from the network, created by agents on other conductors. It is responsible for performing initial checks on the incoming data and persisting it, ready for validation.
- The sys validation workflow `crates/holochain/src/core/workflow/sys_validation_workflow.rs`, enforces common validation logic that is expected to be needed by all applications. The checks it performs are documented in the module documentation for the workflow.
- The app validation workflow `crates/holochain/src/core/workflow/app_validation_workflow.rs`, allows the application's integrity zomes to define extra rules. The required `validate` callback of an integrity zome is dispatched with each DHT op to be validated. Ops either pass validation, are rejected, or wait for dependencies. Once an op has completed validation, it goes to integration.
- The integration workflow `crates/holochain/src/core/workflow/integrate_dht_ops_workflow.rs`, is the final processing step for ops that have completed validation. Ops have either failed sys validation, passed sys validation and failed app validation, or passed both sys and app validation. Integration marks the ops as part of the DHT at that point and they can start being gossiped.

It is critical that workflows handle errors properly, and don't conflict with each other's data state. Content must always be in a state where at least one workflow can progress its state towards being part of the DHT state.

Note that there are subtly different code paths for data that is authored locally, compared with data that is authored on other conductor instances and sent over the network. Differences should be minimized and where possible, diverged code paths should be resolved so that authored data is treated similarly to network-authored data.

## `holochain_data` migration

Holochain data access now centers on `holochain_data`, with remaining higher-level access
spread across `holochain_state`, `holochain_cascade` and `holochain` itself.

This is intended to change and a refactor is in progress. Always prefer following the input given by the user because the refactor is being done in stages but you should help the user stay on track with the intended direction of the refactor.

The remaining goals for the refactor are:
- Keep primitive SQL access in `holochain_data`.
- Keep `holochain_state` as the consumer of `holochain_data`, exposing store-style APIs for compound operations.
- Instead of querying across multiple databases, keep `holochain_cascade` focused on combining access to the DHT store with network requests. That part of the logic will largely remain intact, but the complex traits, transaction handling and data merging operations will be removed.
- The `holochain` crate will access the `holochain_cascade` and `holochain_state` APIs to do its work. There should be no SQL queries remaining in `holochain` outside of tests. This primarily applies to the workflows, which have complex SQL queries that can and should be tested in isolation.
- At a later stage, the `holochain_state` types crate could also be eliminated by figuring out the current circular dependency problems and finding a new home for those types.
