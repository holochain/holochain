# holochain Makefile

# the gha workflow sets this globally
# set this also for local executions so we get the same results
F=RUSTFLAGS="-Dwarnings"

# All default features of binaries excluding mutually exclusive features wasmer_sys & wasmer_wamr
DEFAULT_FEATURES=slow_tests,build_wasms,sqlite-encrypted,hc_demo_cli/build_demo
UNSTABLE_FEATURES=chc,unstable-dpki,unstable-sharding,unstable-warrants,$(DEFAULT_FEATURES)

# mark everything as phony because it doesn't represent a file-system output
.PHONY: default \
	static-all static-fmt static-toml static-clippy static-doc \
	build-workspace-wasmer_sys build-workspace-wasmer_wamr \
	test-workspace-wasmer_sys test-workspace-wasmer_wamr \
	build-workspace-wasmer_sys-unstable \
	test-workspace-wasmer_sys-unstable

# default to running everything (first rule)
default: build-workspace-wasmer_sys \
	test-workspace-wasmer_sys \
	build-workspace-wasmer_wamr \
	test-workspace-wasmer_wamr

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
	$(F) CHK_SQL_FMT=1 cargo clippy --all-targets

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

build-workspace-wasmer_sys-unstable:
	$(F) cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer_sys

build-workspace-wasmer_wamr:
	$(F) cargo build \
		--workspace \
		--locked \
		--all-targets \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_wamr

# execute tests on all crates with wasmer compiler
test-workspace-wasmer_sys:
	cargo install cargo-nextest
	$(F) RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_sys

# executes tests on all crates with wasmer compiler and unstable dpki feature
test-workspace-wasmer_sys-unstable:
	cargo install cargo-nextest
	$(F) RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(UNSTABLE_FEATURES),wasmer_sys

# execute tests on all crates with wasmer interpreter
test-workspace-wasmer_wamr:
	cargo install cargo-nextest
	$(F) RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--locked \
		--no-default-features \
		--features $(DEFAULT_FEATURES),wasmer_wamr
