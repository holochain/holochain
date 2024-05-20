# holochain Makefile

# We have a balance to strike here between having too many CI jobs
# and bundling them up too far so the jobs run out of disk space.
# Try to bundle in groups that use similar dependencies.

# Holochain itself and any crates that pull holochain in as a library dep.
# This list should be kept small, since it builds a lot.
TEST_HOLOCHAIN = \
	holochain \
	hc_demo_cli

# Kitsune crates.
TEST_KITSUNE = \
	kitsune_p2p/bootstrap \
	kitsune_p2p/bootstrap_client \
	kitsune_p2p/dht \
	kitsune_p2p/dht_arc \
	kitsune_p2p/fetch \
	kitsune_p2p/kitsune_p2p \
	kitsune_p2p/mdns \
	kitsune_p2p/proxy \
	kitsune_p2p/timestamp \
	kitsune_p2p/transport_quic \
	kitsune_p2p/types

# Crate dependencies other than kitsune which feed into holochain.
TEST_DEPS = \
	hdk \
	holo_hash \
	hdi \
	mr_bundle \
	holochain_integrity_types \
	holochain_zome_types \
	holochain_types \
	holochain_cascade \
	holochain_conductor_api \
	holochain_p2p \
	holochain_keystore \
	holochain_state \
	holochain_state_types \
	holochain_sqlite \
	holochain_trace \
	holochain_websocket \
	holochain_util \
	holochain_metrics \
	holochain_nonce \
	holochain_secure_primitive \
	holochain_conductor_services \
	holochain_terminal

# An additional test bucket for testing utility crates and crates
# that are siblings to holochain but don't depend on the library directly.
TEST_MISC = \
	hc \
	hc_bundle \
	hc_sleuth \
	hc_run_local_services \
	hc_service_check \
	aitia \
	fixt \
	fixt/test \
	mock_hdi \
	test_utils/wasm \
	test_utils/wasm_common \
	holochain_diagnostics \
	diagnostic_tests

.PHONY: all test-all $(TEST_HOLOCHAIN) $(TEST_KITSUNE) $(TEST_DEPS) $(TEST_MISC) hc_sandbox hdk_derive

all: test-all

test-all: test-holochain test-kitsune test-deps test-misc hc_sandbox

test-holochain: $(TEST_HOLOCHAIN)

test-kitsune: $(TEST_KITSUNE)

test-deps: $(TEST_DEPS) hdk_derive

test-misc: $(TEST_MISC)

$(TEST_HOLOCHAIN) $(TEST_KITSUNE) $(TEST_DEPS) $(TEST_MISC):
	cargo install cargo-nextest
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" cargo build -j4 \
		--all-features --all-targets \
		--profile fast-test
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" RUST_BACKTRACE=1 cargo nextest run \
		--build-jobs 4 \
		--cargo-profile fast-test \
		--all-features

# hdk_derive is a special case because of
# https://github.com/nextest-rs/nextest/issues/267
# essentially nextest doesn't work, we have to use plain-old cargo test
hdk_derive:
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" cargo build -j4 \
		--all-features --all-targets \
		--profile fast-test
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" RUST_BACKTRACE=1 cargo test -j4 \
		--all-features \
		--profile fast-test

# hc_sandbox is a special case because it requires binaries on the path
# NOTE - this one must be its own top-level github action job
#        since it uses so much disk space
# TODO - while the cargo install-s below technically work, it'd be much
#        more appropriate for the test to use binaries out of the target
#        directory by default
hc_sandbox:
	cargo install cargo-nextest
	cargo install --force --path crates/holochain
	cargo install --force --path crates/hc
	cargo install --force --path crates/hc_sandbox
	cd crates/hc_sandbox && \
		RUSTFLAGS="-Dwarnings" RUST_BACKTRACE=1 cargo nextest run \
		--build-jobs 4 \
		--cargo-profile fast-test \
		--all-features
