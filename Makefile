# holochain Makefile

# mark everything as phony because it doesn't represent a file-system output
.PHONY: default test-workspace

# default to running everything (first rule)
default: build-workspace test-workspace

# Build all targets.
# This not only builds the test binaries for usage by `test-workspace`,
# but also ensures targets like benchmarks remain buildable.
# NOTE: excludes must match test-workspace nextest params,
#       otherwise some rebuilding will occur due to resolver = "2"
build-workspace:
	RUSTFLAGS="-Dwarnings" \
		cargo build \
		--workspace \
		--exclude holochain_cli_sandbox \
		--exclude hdk_derive \
		--locked \
		--all-features --all-targets

# Execute tests on all creates.
# TODO - make hc_sandbox able to run binaries out of target/debug
test-workspace:
	cargo install cargo-nextest
	RUSTFLAGS="-Dwarnings" RUST_BACKTRACE=1 \
		cargo nextest run \
		--workspace \
		--exclude holochain_cli_sandbox \
		--exclude hdk_derive \
		--locked \
		--all-features
	# hdk_derive cannot currently be tested via nextest
	# https://github.com/nextest-rs/nextest/issues/267
	RUSTFLAGS="-Dwarnings" \
		cargo build \
		--manifest-path crates/hdk_derive/Cargo.toml \
		--locked \
		--all-features --all-targets
	RUSTFLAGS="-Dwarnings" RUST_BACKTRACE=1 \
		cargo test \
		--manifest-path crates/hdk_derive/Cargo.toml \
		--locked \
		--all-features
