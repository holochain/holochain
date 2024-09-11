# holochain Makefile

# the gha workflow sets this globally
# set this also for local executions so we get the same results
F=RUSTFLAGS="-Dwarnings"

# All default features of binaries excluding mutually exclusive features wasmer_sys & wasmer_wamr
DEFAULT_FEATURES=chc,slow_tests,build_wasms,sqlite-encrypted,hc_demo_cli/build_demo

# mark everything as phony because it doesn't represent a file-system output
.PHONY: default \
	static-all static-fmt static-toml static-clippy static-doc \
	build-workspace test-workspace

# default to running everything (first rule)
default: build-workspace test-workspace

# execute all static code validation
static-all: static-fmt static-toml static-clippy static-doc

# ensure committed code is formatted properly
static-fmt:
	$(F) cargo fmt --check

# lint our toml files
static-toml:
	cargo install taplo-cli@0.9.0
	$(F) taplo format --check ./*.toml
	$(F) taplo format --check ./crates/**/*.toml

# ensure our chosen style lints are followed
static-clippy:
	$(F) cargo clippy --all-targets

# ensure we can build the docs
static-doc:
	$(F) cargo doc

# build all targets
# this not only builds the test binaries for usage by `test-workspace`,
# but also ensures targets like benchmarks remain buildable.
# NOTE: excludes must match test-workspace nextest params,
#       otherwise some rebuilding will occur due to resolver = "2"
build-workspace-wasmer_sys:
	$(F) cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_sys

build-workspace-wasmer_wamr:
	$(F) cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_wamr

# execute tests on all creates
test-workspace-wasmer_sys:
	cargo install cargo-nextest
	$(F) RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_sys

test-workspace-wasmer_wamr:
	cargo install cargo-nextest
	$(F) RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_wamr
