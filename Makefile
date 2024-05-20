# holochain Makefile

TEST_HOLOCHAIN = holochain

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

TEST_DEPS = \
	hdk \
	hdk_derive \
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

TEST_HC = \
	hc \
	hc_bundle \
	hc_sandbox \
	hc_sleuth \
	hc_run_local_services \
	hc_demo_cli \
	hc_service_check

TEST_MISC = \
	aitia \
	fixt \
	fixt/test \
	mock_hdi \
	holochain \
	test_utils/wasm \
	test_utils/wasm_common \
	holochain_diagnostics \
	diagnostic_tests

.PHONY: all test-all $(TEST_HOLOCHAIN) $(TEST_KITSUNE) $(TEST_DEPS) $(TEST_HC) $(TEST_MISC)

all: test-all

test-all: test-holochain test-kitsune test-deps test-hc test-misc

test-holochain: $(TEST_HOLOCHAIN)

test-kitsune: $(TEST_KITSUNE)

test-deps: $(TEST_DEPS)

test-hc: $(TEST_HC)

test-misc: $(TEST_MISC)

$(TEST_HOLOCHAIN) $(TEST_KITSUNE) $(TEST_DEPS) $(TEST_HC) $(TEST_MISC):
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
