# holochain Makefile

# We have a balance to strike here between having too many CI jobs
# and bundling them up too far so the jobs run out of disk space.
# Try to bundle in groups that use similar dependencies.

# Holochain itself and any crates that pull holochain in as a library dep.
# This list should be kept small, since it builds a lot.
TEST_HOLOCHAIN = \
	holochain \
	hc_demo_cli \
	holochain_diagnostics \
	diagnostic_tests

# Large holochain dependencies that pull in big libraries like tx5.
TEST_DEPS = \
	kitsune_p2p/bootstrap \
	kitsune_p2p/bootstrap_client \
	kitsune_p2p/fetch \
	kitsune_p2p/kitsune_p2p \
	kitsune_p2p/mdns \
	kitsune_p2p/proxy \
	kitsune_p2p/transport_quic \
	kitsune_p2p/types \
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
	holochain_conductor_services \
	holochain_terminal \
	holochain_websocket \
	test_utils/wasm \
	hc \
	hc_bundle \
	hc_run_local_services \
	hc_sleuth

# Small support crates with small dependency requirements
TEST_MISC = \
	kitsune_p2p/dht \
	kitsune_p2p/dht_arc \
	kitsune_p2p/timestamp \
	hdk \
	holo_hash \
	hdi \
	mr_bundle \
	hc_service_check \
	aitia \
	fixt \
	fixt/test \
	mock_hdi \
	test_utils/wasm_common \
	holochain_trace \
	holochain_metrics \
	holochain_util \
	holochain_nonce \
	holochain_secure_primitive

# The set of tests that require holochain binaries on the path in order to run
TEST_BIN = \
	hc_sandbox

# mark everything as phony because it doesn't represent a file-system output
.PHONY: all test-all $(TEST_HOLOCHAIN) $(TEST_DEPS) $(TEST_MISC) $(TEST_BIN) test-holochain test-deps test-misc test-bin install-bin hdk_derive

# default to running everything (first rule)
default: test-all

# run all the unit test sets
test-all: test-holochain test-deps test-misc test-bin

# run the tests that result in a full build of the holochain library
test-holochain: $(TEST_HOLOCHAIN)

# run the set of tests that use significant deps (such as tx5)
test-deps: $(TEST_DEPS) hdk_derive

# run the set of tests of lighter-weight misc crates
test-misc: $(TEST_MISC)

# run the tests that depend on binaries in the path
test-bin: install-bin $(TEST_BIN)

# TODO - while the cargo install-s below technically work, it'd be much
#        more appropriate for the test to use binaries out of the target
#        directory by default
install-bin:
	cargo install --force --path crates/holochain
	cargo install --force --path crates/hc
	cargo install --force --path crates/hc_sandbox

# the unit test rule - first builds all targets to ensure that things
# like benchmarks at least remain build-able. Then runs nextest tests
$(TEST_HOLOCHAIN) $(TEST_KITSUNE) $(TEST_DEPS) $(TEST_MISC) $(TEST_BIN):
	cargo install cargo-nextest
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" \
		cargo build -j4 \
		--locked \
		--all-features --all-targets \
		--profile fast-test
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" RUST_BACKTRACE=1 \
		cargo nextest run \
		--locked \
		--build-jobs 4 \
		--cargo-profile fast-test \
		--all-features

# hdk_derive is a special case because of
# https://github.com/nextest-rs/nextest/issues/267
# essentially nextest doesn't work, we have to use plain-old cargo test
hdk_derive:
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" \
		cargo build -j4 \
		--locked \
		--all-features --all-targets \
		--profile fast-test
	cd crates/$@ && \
		RUSTFLAGS="-Dwarnings" RUST_BACKTRACE=1 \
		cargo test -j4 \
		--locked \
		--all-features \
		--profile fast-test
