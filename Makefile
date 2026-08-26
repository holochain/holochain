# holochain Makefile

# All default features of binaries excluding mutually exclusive features wasmer-sys-cranelift & wasmer-wasmi
DEFAULT_FEATURES=slow_tests,build_wasms,encryption
UNSTABLE_FEATURES=unstable-sharding,unstable-functions,unstable-migration,$(DEFAULT_FEATURES)

# mark everything as phony because it doesn't represent a file-system output
.PHONY: default \
	static-all static-fmt static-toml static-clippy static-clippy-unstable \
	static-doc build-workspace-wasmer-sys-cranelift build-workspace-wasmer-wasmi \
	build-workspace-wasmer-sys-llvm test-workspace-wasmer-sys-cranelift \
	test-workspace-wasmer-sys-llvm test-workspace-wasmer-wasmi \
	build-workspace-wasmer-sys-cranelift-unstable \
	test-workspace-wasmer-sys-cranelift-unstable \
	toml-fix ts-bindings ts-bindings-test

# default to running everything (first rule)
default: build-workspace-wasmer-sys-cranelift \
	test-workspace-wasmer-sys-cranelift \
	build-workspace-wasmer-wasmi \
	test-workspace-wasmer-wasmi

# execute all static code validation
static-all: static-fmt static-toml static-clippy static-clippy-unstable static-doc

# ensure committed code is formatted properly
static-fmt:
	cargo fmt --check

# lint our toml files
# `--locked` pins taplo's published lockfile: an unlocked install resolves the
# newest semver-compatible deps, and a recent `time` release fails to compile
# (E0119) on our toolchain, breaking `cargo install taplo-cli@0.10.0`.
static-toml:
	cargo install taplo-cli@0.10.0 --locked
	taplo format --check ./*.toml
	taplo format --check ./crates/**/*.toml

# fix our toml files
toml-fix:
	cargo install taplo-cli@0.10.0 --locked
	taplo format ./*.toml
	taplo format ./crates/**/*.toml

# ensure our chosen style lints are followed
static-clippy:
	CHK_SQL_FMT=1 cargo clippy --all-targets --features $(DEFAULT_FEATURES)

static-clippy-unstable:
	CHK_SQL_FMT=1 cargo clippy --all-targets --features $(UNSTABLE_FEATURES)

# ensure we can build the docs
# --no-deps skips generating HTML for third-party dependencies, which is the
# bulk of the work; we only care that our own crates' docs build cleanly.
static-doc:
	RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps

# build all targets
# this not only builds the test binaries for usage by `test-workspace`,
# but also ensures targets like benchmarks remain buildable.
# NOTE: excludes must match test-workspace nextest params,
#       otherwise some rebuilding will occur due to resolver = "2"
build-workspace-wasmer-sys-cranelift:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-cranelift

build-workspace-wasmer-sys-cranelift-unstable:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer-sys-cranelift

build-workspace-wasmer-sys-llvm:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-llvm

build-workspace-wasmer-wasmi:
	cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-wasmi

# execute tests on all crates with the cranelift wasmer compiler and iroh transport
test-workspace-wasmer-sys-cranelift:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-cranelift

# execute tests on all crates with the LLVM wasmer compiler and iroh transport
test-workspace-wasmer-sys-llvm:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-sys-llvm

# executes tests on all crates with wasmer compiler
test-workspace-wasmer-sys-cranelift-unstable:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer-sys-cranelift

# execute tests on all crates with wasmer interpreter
test-workspace-wasmer-wasmi:
	RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer-wasmi

# Writes the TypeScript binding tree with `hc export-ts-bindings`, which
# stages the whole tree in one process (ts-rs only merges declarations sharing
# an output file within a single process) before replacing the output
# directory. `hc`'s `ts_rs` feature builds in the subcommand; it is off by
# default so ordinary workspace builds don't compile the type crates with
# `ts_rs`. `unstable-countersigning` implies `ts_rs` and additionally adds the
# countersigning app API. See holochain_conductor_api::export_ts_bindings.
ts-bindings:
	cargo run -p holochain_cli --locked --features unstable-countersigning -- \
		export-ts-bindings --out-dir $(or $(TS_BINDINGS_DIR),./bindings)

# Runs the export, then the hc integration tests, with the countersigning app
# API enabled. The ts-bindings prerequisite's real purpose here is to build
# hc with the right features first, so the tests under crates/hc/tests/ pick
# up that binary via CARGO_BIN_EXE_hc instead of a stale one. The ts_rs-gated
# tests of the type crates only run here, not in test-workspace: hc's ts_rs
# feature is off by default there, so the type crates compile without it.
ts-bindings-test: ts-bindings
	cargo test -p holochain_cli --features unstable-countersigning

clean:
	cargo clean
    # Remove untracked .dna files
	git ls-files -z --others --ignored --exclude-standard -- '*.dna' | xargs -0 rm --
    # Remove untracked .happ files
	git ls-files -z --others --ignored --exclude-standard '*.happ' | xargs -0 rm --
