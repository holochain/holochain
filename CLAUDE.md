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
- **Public API docs**: `///` rustdoc on public items; module/crate docs should describe structure. Follow [rustdoc's guidance](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html#documenting-components): keep the first line a short, one-sentence summary — everything up to the first blank `///` line is reused as the summary in module/search listings — then a blank `///` line before any further detail.
- **Commits**: Conventional Commits (`feat:`, `fix:`, `refactor:`, etc.), bodies wrapped near 72 chars.
- **Changelog**: Write changelog entries only to
  `crates/holochain/CHANGELOG.md`. Include only information relevant to end
  users; omit internal implementation details and changes with no user-visible
  impact.
- **PRs**: branch off `develop`; changes are squash merged into `develop`; changes go from `develop` → `main` at release time and `main` should always be ignored for development.

### ts_rs client export

Some crates (`holo_hash`, `holochain_timestamp`, `holochain_nonce`,
`mr_bundle`, `holochain_integrity_types`, `holochain_state_types`,
`holochain_zome_types`, `holochain_types`, `holochain_conductor_api`) carry
an opt-in `ts_rs` cargo feature that derives TypeScript bindings
(`ts_rs::TS`) for the conductor's wire API, consumed by
`holochain-client-js`. It is off by default; ordinary builds and
`make test-workspace` never enable it.

**Criterion — does a type need a TS export?** Only if it is reachable from
the wire surface: transitively referenced (directly or via a field, enum
variant, generic parameter, or type alias) from `AdminRequest`,
`AdminResponse`, `AppRequest`, `AppResponse`, or `Signal`
(`holochain_conductor_api`, `holochain_zome_types`). This is not a
hand-maintained list — the compiler enforces it. Deriving `TS` on the four
entry points (plus `Signal`) forces every type they transitively reference
to implement `TS` too, so `cargo build -p holochain_conductor_api --features
ts_rs` fails on any newly reachable type that hasn't been given one yet.
Run that build after adding or changing a field on any admin/app
request/response/signal type; a "trait `ts_rs::TS` is not implemented"
error names exactly what still needs handling. Types outside this closure
(workflow-internal state, DB row types, other host-only structures) do not
need a TS export even if they resemble an exported type.

**How to add the derive**, once a type is in scope:

- Plain structs/enums:
  `#[cfg_attr(feature = "ts_rs", derive(ts_rs::TS))]` +
  `#[cfg_attr(feature = "ts_rs", ts(export, export_to = "…"))]`, file chosen
  to match the type's interface (`api/admin/types.ts`, `api/app/types.ts`,
  `hdk/*.ts`, or the shared `types.ts`).
- Byte fields (`serde_bytes`, `bytes::Bytes`, `serialize_bytes`):
  `#[cfg_attr(feature = "ts_rs", ts(type = "Uint8Array"))]`.
- Opaque JSON/YAML payloads: `#[cfg_attr(feature = "ts_rs", ts(type =
  "unknown"))]`.
- Types with a manual/hand-written `serde` impl (hashes, newtype wrappers,
  Rust `type` aliases) cannot derive `TS` — give them a hand-written impl
  (see `holo_hash/src/ts.rs`) or the `ts_alias!` macro instead of guessing
  at an override.
- Client→conductor fields the wire lets a caller omit — any `Option` field
  (serde's derive treats a missing `Option` as `None`) and any field with
  `#[serde(default)]` — carry
  `#[cfg_attr(feature = "ts_rs", ts(optional = nullable))]` so the generated
  TS marks them `field?: …`. This also applies to types that appear in both
  requests and responses (e.g. the app/DNA manifests): response readers then
  see `?` on fields the conductor always serializes, an accepted trade-off —
  do not remove the annotation to "fix" the response side. Response-only
  types keep required fields.
- Each crate forwards `ts_rs` to the in-set deps it needs;
  never enable `ts_rs` in a crate's own dev-dependency self-reference, or
  export tests run during normal `make test-workspace`.
- Manual/alias impls are exported by each crate's
  `ts_rs`-gated `export_ts_bindings` function (crate root or `ts` module),
  which chains its in-set deps' functions; when adding one, register it
  there. The whole tree is written by the `export-ts-bindings` binary in the
  `hc` crate (`make ts-bindings`, which runs `cargo run -p holochain_cli
  --features ts_rs,unstable-countersigning --bin export-ts-bindings`) because
  ts-rs only merges declarations sharing an output file within one process —
  never split the export across per-crate loops. The binary is gated on
  `required-features = ["ts_rs"]`, so builds that don't enable the feature
  skip it. It stages the tree in a temp directory and only replaces
  `TS_RS_EXPORT_DIR` once the export succeeds, so a failed export never
  leaves it half-written. `make ts-bindings-test` also runs the
  `holochain_conductor_api` ts_rs-gated tests (the wire-format smoke test and
  the tag-injection helper's unit tests) — CI runs this target since ordinary
  `make test-workspace` never enables the `ts_rs` feature.
- The binary lives in `hc` rather than `holochain_conductor_api` so that
  holochain-client-js can build it from a pinned revision through Holonix,
  whose `hc` package appends overridden arguments to a hard-wired
  `--manifest-path crates/hc/Cargo.toml --bin hc`; a bin in another crate
  cannot be reached that way. `hc` also pulls in neither the `holochain`
  crate nor a wasmer backend, so the export does not compile a WASM engine.
- The binary sets the TypeScript dialect (`number` for 64-bit integers, `.js`
  import extensions) in code, taking only the output directory from
  `TS_RS_EXPORT_DIR`. The matching `TS_RS_*` entries in `.cargo/config.toml`
  reach only processes Cargo spawns from this workspace, which the binary is
  not when the client builds and runs it from its own flake; keep the two in
  sync.
- The export build enables `unstable-countersigning` on top of `ts_rs`, so
  the countersigning app API and its session state types both reach the
  bindings. Types and fields gated behind the remaining `unstable-*`
  features are compiled out of the export build and absent from the
  bindings.
- Zome call return types (`Record`, `Details`, `Link`, `AgentActivity`) are
  outside the wire closure — a zome call carries them as an opaque
  `ExternIO` the client decodes itself — but are exported anyway, because
  the client needs them to type zome call results. Register them in the
  owning crate's `export_ts_bindings` like any other unreachable type.
- After the export, `holochain_conductor_api` rewrites every generated file
  so each exported declaration carries a TSDoc `@public` release tag
  (appended to an existing doc block, or added as a minimal block) — the
  holochain-client-js documentation build (api-extractor) errors on
  exports without one.

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
