# holochain Makefile

CRATES = \
	aitia \
	fixt \
	fixt/test \
	hdk \
	hdk_derive \
	holo_hash \
	hdi \
	mock_hdi \
	mr_bundle \
	holochain_integrity_types \
	holochain_zome_types \
	holochain_types \
	holochain \
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
	hc \
	hc_bundle \
	hc_sandbox \
	hc_sleuth \
	hc_run_local_services \
	hc_demo_cli \
	hc_service_check \
	holochain_terminal \
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
	kitsune_p2p/types \
	test_utils/wasm \
	test_utils/wasm_common \
	holochain_diagnostics \
	diagnostic_tests

.PHONY: all test-all $(CRATES)

all: test-all

test-all: $(CRATES)

$(CRATES):
	cd crates/$@ && \
		cargo build -j4 \
		--all-features --all-targets \
		--profile fast-test
	cd crates/$@ && \
		RUST_BACKTRACE=1 cargo test -j4 \
		--all-features \
		--profile fast-test \
		-- --test-threads 1 --nocapture
