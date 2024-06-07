# holochain Makefile

# the gha workflow sets this globally
# set this also for local executions so we get the same results
F=RUSTFLAGS="-Dwarnings"

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
	# TODO fix lints and switch to `$(F) cargo clippy --all-targets`
	$(F) cargo clippy

# ensure we can build the docs
static-doc:
	$(F) cargo doc

# build all targets
# this not only builds the test binaries for usage by `test-workspace`,
# but also ensures targets like benchmarks remain buildable.
# NOTE: excludes must match test-workspace nextest params,
#       otherwise some rebuilding will occur due to resolver = "2"
build-workspace:
	$(F) cargo build \
		--workspace \
		--exclude holochain_cli_sandbox \
		--exclude hdk_derive \
		--locked \
		--all-features --all-targets

# execute tests on all creates
# TODO - make hc_sandbox able to run binaries out of target/debug
test-workspace:
	cargo install cargo-nextest
	$(F) RUST_BACKTRACE=1 cargo nextest run \
		--workspace \
		--exclude holochain_cli_sandbox \
		--exclude hdk_derive \
		--locked \
		--all-features
	# hdk_derive cannot currently be tested via nextest
	# https://github.com/nextest-rs/nextest/issues/267
	$(F) cargo build \
		--manifest-path crates/hdk_derive/Cargo.toml \
		--locked \
		--all-features --all-targets
	$(F) RUST_BACKTRACE=1 cargo test \
		--manifest-path crates/hdk_derive/Cargo.toml \
		--locked \
		--all-features
