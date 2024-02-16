# Changelog

This file conveniently consolidates all of the crates individual CHANGELOG.md files and groups them by timestamps at which crates were released. The file is updated every time one or more crates are released.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# \[Unreleased\]

# 20240216.150955

## [holochain\_cli-0.2.7-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.7-rc.0)

## [holochain-0.2.7-rc.0](crates/holochain/CHANGELOG.md#0.2.7-rc.0)

- Added `lair_keystore_version_req` to the output of `--build-info` for Holochain.

## [holochain\_cli\_bundle-0.2.7-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_cli\_sandbox-0.2.7-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_cascade-0.2.7-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_conductor\_api-0.2.7-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_test\_wasm\_common-0.2.7-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_wasm\_test\_utils-0.2.7-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.7-rc.0)

## [hdk-0.2.7-rc.0](crates/hdk/CHANGELOG.md#0.2.7-rc.0)

- Added `create_clone_cell`, `disable_clone_cell`, `enable_clone_cell` and `delete_clone_cell` functionality to the HDK. This was previously only available on the admin interface of Holochain which shouldn’t be used by apps. Exposing this functionality through the HDK allows happ developers to manage clones from their backend code without having to worry about their apps breaking when more security is added to the admin interface. The only restriction on the use of these methods is that they will not permit you to create clones in another app. You can create clones of any cell within the app you make the host function calls from.

## [holochain\_state-0.2.7-rc.0](crates/holochain_state/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_p2p-0.2.7-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_types-0.2.7-rc.0](crates/holochain_types/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_keystore-0.2.7-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_sqlite-0.2.7-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.7-rc.0)

## [holochain\_zome\_types-0.2.7-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.7-rc.0)

# 20240206.184429

## [holochain\_cli-0.2.6](crates/holochain_cli/CHANGELOG.md#0.2.6)

## [holochain-0.2.6](crates/holochain/CHANGELOG.md#0.2.6)

## [holochain\_cli\_bundle-0.2.6](crates/holochain_cli_bundle/CHANGELOG.md#0.2.6)

## [holochain\_cli\_run\_local\_services-0.2.6](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.6)

## [holochain\_cli\_sandbox-0.2.6](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.6)

## [holochain\_cascade-0.2.6](crates/holochain_cascade/CHANGELOG.md#0.2.6)

## [holochain\_conductor\_api-0.2.6](crates/holochain_conductor_api/CHANGELOG.md#0.2.6)

## [holochain\_metrics-0.2.6](crates/holochain_metrics/CHANGELOG.md#0.2.6)

## [holochain\_test\_wasm\_common-0.2.6](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.6)

## [holochain\_wasm\_test\_utils-0.2.6](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.6)

## [holochain\_websocket-0.2.6](crates/holochain_websocket/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_bootstrap-0.1.6](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.6)

## [hdk-0.2.6](crates/hdk/CHANGELOG.md#0.2.6)

## [holochain\_state-0.2.6](crates/holochain_state/CHANGELOG.md#0.2.6)

## [hdi-0.3.6](crates/hdi/CHANGELOG.md#0.3.6)

## [holochain\_p2p-0.2.6](crates/holochain_p2p/CHANGELOG.md#0.2.6)

## [hdk\_derive-0.2.6](crates/hdk_derive/CHANGELOG.md#0.2.6)

## [holochain\_types-0.2.6](crates/holochain_types/CHANGELOG.md#0.2.6)

## [holochain\_keystore-0.2.6](crates/holochain_keystore/CHANGELOG.md#0.2.6)

## [holochain\_sqlite-0.2.6](crates/holochain_sqlite/CHANGELOG.md#0.2.6)

## [mr\_bundle-0.2.6](crates/mr_bundle/CHANGELOG.md#0.2.6)

## [holochain\_zome\_types-0.2.6](crates/holochain_zome_types/CHANGELOG.md#0.2.6)

## [kitsune\_p2p-0.2.6](crates/kitsune_p2p/CHANGELOG.md#0.2.6)

## [holochain\_integrity\_types-0.2.6](crates/holochain_integrity_types/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_fetch-0.2.6](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_mdns-0.2.6](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_proxy-0.2.6](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.6)

## [holo\_hash-0.2.6](crates/holo_hash/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_transport\_quic-0.2.6](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.6)

## [fixt-0.2.6](crates/fixt/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_types-0.2.6](crates/kitsune_p2p_types/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_block-0.2.6](crates/kitsune_p2p_block/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_dht-0.2.6](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_bin\_data-0.2.6](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_timestamp-0.2.6](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.6)

## [holochain\_util-0.2.6](crates/holochain_util/CHANGELOG.md#0.2.6)

## [kitsune\_p2p\_dht\_arc-0.2.6](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.6)

## [holochain\_trace-0.2.6](crates/holochain_trace/CHANGELOG.md#0.2.6)

# 20240201.135624

## [holochain\_cli-0.2.6-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.6-rc.0)

## [holochain-0.2.6-rc.0](crates/holochain/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_cli\_bundle-0.2.6-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_cli\_run\_local\_services-0.2.6-rc.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_cli\_sandbox-0.2.6-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_cascade-0.2.6-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_conductor\_api-0.2.6-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_test\_wasm\_common-0.2.6-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_wasm\_test\_utils-0.2.6-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_websocket-0.2.6-rc.0](crates/holochain_websocket/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_bootstrap-0.1.6-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.6-rc.0)

## [hdk-0.2.6-rc.0](crates/hdk/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_state-0.2.6-rc.0](crates/holochain_state/CHANGELOG.md#0.2.6-rc.0)

## [hdi-0.3.6-rc.0](crates/hdi/CHANGELOG.md#0.3.6-rc.0)

## [holochain\_p2p-0.2.6-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.6-rc.0)

## [hdk\_derive-0.2.6-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_types-0.2.6-rc.0](crates/holochain_types/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_keystore-0.2.6-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_sqlite-0.2.6-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.6-rc.0)

## [holochain\_zome\_types-0.2.6-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p-0.2.6-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.6-rc.0)

- Performance improvement by reducing the number of `query_agents` calls used by Kitsune. The host (Holochain conductor) responds to these queries using an in-memory store which is fast but all the queries go through the `ghost_actor` so making an excessive number of calls for the same information reduces the availability of the host for other calls. For a test which sets up 10 spaces (equivalent to a happ running on the host) this change takes the number of host queries for agent info from ~13k to ~1.4k. The removed calls were largely redundant since Kitsune refreshes agent info every 1s anyway so it shouldn’t need to make many further calls between refreshes.

## [holochain\_integrity\_types-0.2.6-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_fetch-0.2.6-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.6-rc.0)

- Enhance source backoff logic. The fetch pool used to give a source a 5 minute pause if it failed to serve an op before using the source again. Now the failures to serve by sources is tracked across the pool. Sources that fail too often will be put on a backoff to give them a chance to deal with their current workload before we use them again. For hosts that continue to not respond they will be dropped as sources for ops. Ops that end up with no sources will be dropped from the fetch pool. This means that we can stop using resources on ops we will never be able to fetch. If a source appears who is capable of serving the missing ops then they should be re-added to the fetch pool.

## [kitsune\_p2p\_proxy-0.2.6-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.6-rc.0)

## [holo\_hash-0.2.6-rc.0](crates/holo_hash/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.6-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_types-0.2.6-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_block-0.2.6-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_dht-0.2.6-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_bin\_data-0.2.6-rc.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_timestamp-0.2.6-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.6-rc.0)

## [kitsune\_p2p\_dht\_arc-0.2.6-rc.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.6-rc.0)

# 20240131.130303

## [holochain\_cli-0.2.5](crates/holochain_cli/CHANGELOG.md#0.2.5)

## [holochain-0.2.5](crates/holochain/CHANGELOG.md#0.2.5)

## [holochain\_cli\_bundle-0.2.5](crates/holochain_cli_bundle/CHANGELOG.md#0.2.5)

## [holochain\_cli\_run\_local\_services-0.2.5](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.5)

## [holochain\_cli\_sandbox-0.2.5](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.5)

## [holochain\_cascade-0.2.5](crates/holochain_cascade/CHANGELOG.md#0.2.5)

## [holochain\_conductor\_api-0.2.5](crates/holochain_conductor_api/CHANGELOG.md#0.2.5)

## [holochain\_metrics-0.2.5](crates/holochain_metrics/CHANGELOG.md#0.2.5)

## [holochain\_test\_wasm\_common-0.2.5](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.5)

## [holochain\_wasm\_test\_utils-0.2.5](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.5)

## [holochain\_websocket-0.2.5](crates/holochain_websocket/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_bootstrap-0.1.5](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.5)

## [hdk-0.2.5](crates/hdk/CHANGELOG.md#0.2.5)

## [holochain\_state-0.2.5](crates/holochain_state/CHANGELOG.md#0.2.5)

## [hdi-0.3.5](crates/hdi/CHANGELOG.md#0.3.5)

## [holochain\_p2p-0.2.5](crates/holochain_p2p/CHANGELOG.md#0.2.5)

## [hdk\_derive-0.2.5](crates/hdk_derive/CHANGELOG.md#0.2.5)

## [holochain\_types-0.2.5](crates/holochain_types/CHANGELOG.md#0.2.5)

## [holochain\_keystore-0.2.5](crates/holochain_keystore/CHANGELOG.md#0.2.5)

- Update Lair version to 0.4.2. This change just removes `#![deny(warnings)]` from Lair so that it won’t prevent builds when new versions of Clippy introduce new lint rules. There are no functional changes to Lair or its API.

## [holochain\_sqlite-0.2.5](crates/holochain_sqlite/CHANGELOG.md#0.2.5)

## [mr\_bundle-0.2.5](crates/mr_bundle/CHANGELOG.md#0.2.5)

## [holochain\_zome\_types-0.2.5](crates/holochain_zome_types/CHANGELOG.md#0.2.5)

## [kitsune\_p2p-0.2.5](crates/kitsune_p2p/CHANGELOG.md#0.2.5)

## [holochain\_integrity\_types-0.2.5](crates/holochain_integrity_types/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_fetch-0.2.5](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_mdns-0.2.5](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_proxy-0.2.5](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.5)

## [holo\_hash-0.2.5](crates/holo_hash/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_transport\_quic-0.2.5](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.5)

## [fixt-0.2.5](crates/fixt/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_types-0.2.5](crates/kitsune_p2p_types/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_block-0.2.5](crates/kitsune_p2p_block/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_dht-0.2.5](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_bin\_data-0.2.5](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_timestamp-0.2.5](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.5)

## [holochain\_util-0.2.5](crates/holochain_util/CHANGELOG.md#0.2.5)

## [kitsune\_p2p\_dht\_arc-0.2.5](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.5)

## [holochain\_trace-0.2.5](crates/holochain_trace/CHANGELOG.md#0.2.5)

# 20240125.194538

## [holochain\_cli-0.2.5-rc.1](crates/holochain_cli/CHANGELOG.md#0.2.5-rc.1)

## [holochain-0.2.5-rc.1](crates/holochain/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_cli\_bundle-0.2.5-rc.1](crates/holochain_cli_bundle/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_cli\_run\_local\_services-0.2.5-rc.1](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_cli\_sandbox-0.2.5-rc.1](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_cascade-0.2.5-rc.1](crates/holochain_cascade/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_conductor\_api-0.2.5-rc.1](crates/holochain_conductor_api/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_wasm\_test\_utils-0.2.5-rc.1](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.5-rc.1)

## [kitsune\_p2p\_bootstrap-0.1.5-rc.1](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.5-rc.1)

## [holochain\_state-0.2.5-rc.1](crates/holochain_state/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_p2p-0.2.5-rc.1](crates/holochain_p2p/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_types-0.2.5-rc.1](crates/holochain_types/CHANGELOG.md#0.2.5-rc.1)

## [holochain\_keystore-0.2.5-rc.1](crates/holochain_keystore/CHANGELOG.md#0.2.5-rc.1)

- Update Lair version to 0.4.1, see the Lair changelog [here](https://github.com/holochain/lair/blob/main/crates/lair_keystore/CHANGELOG.md#041).

## [holochain\_sqlite-0.2.5-rc.1](crates/holochain_sqlite/CHANGELOG.md#0.2.5-rc.1)

## [mr\_bundle-0.2.5-rc.0](crates/mr_bundle/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p-0.2.5-rc.1](crates/kitsune_p2p/CHANGELOG.md#0.2.5-rc.1)

- Minor optimisation when delegate broadcasting ops, the delegated broadcasts will now avoid connecting back to the source. There is currently no way to prevent other agents that were delegated to from connecting to each other but this change takes care of one case.

## [kitsune\_p2p\_fetch-0.2.5-rc.1](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.5-rc.1)

## [kitsune\_p2p\_proxy-0.2.5-rc.1](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.5-rc.1)

## [kitsune\_p2p\_transport\_quic-0.2.5-rc.1](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.5-rc.1)

## [kitsune\_p2p\_types-0.2.5-rc.1](crates/kitsune_p2p_types/CHANGELOG.md#0.2.5-rc.1)

# 20240120.122931

## [holochain\_cli-0.2.5-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.5-rc.0)

## [holochain-0.2.5-rc.0](crates/holochain/CHANGELOG.md#0.2.5-rc.0)

- Fix: Wasmer cache was deserializing modules for every zome call which slowed them down. Additionally the instance cache that was supposed to store callable instances of modules was not doing that correctly. A cache for deserialized modules has been re-introduced and the instance cache was removed, following recommendation from the wasmer team regarding caching.
- Fix: Runtime engines to execute zome fns did not live long enough and were producing `RuntimeError: out of bounds memory access`. Engines are kept for the lifetime of the conductor process now.
- Fix: Call contexts of internal callbacks like `validate` were not cleaned up from an in-memory map. Now external as well as internal callbacks remove the call contexts from memory. This is covered by a test.
- Make sqlite-encrypted a default feature
- Sys validation will no longer check the integrity with the previous action for StoreRecord or StoreEntry ops. These ‘store record’ checks are now only done for RegisterAgentActivity ops which we are sent when we are responsible for validating an agents whole chain. This avoids fetching and caching ops that we don’t actually need.
- Sys validation will now validate that a DeleteLink points to an action which is a CreateLink through the `link_add_address` of the delete.

## [holochain\_cli\_bundle-0.2.5-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_cli\_run\_local\_services-0.2.5-rc.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_cli\_sandbox-0.2.5-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_cascade-0.2.5-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_conductor\_api-0.2.5-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_test\_wasm\_common-0.2.5-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_wasm\_test\_utils-0.2.5-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_websocket-0.2.5-rc.0](crates/holochain_websocket/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_bootstrap-0.1.5-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.5-rc.0)

## [hdk-0.2.5-rc.0](crates/hdk/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_state-0.2.5-rc.0](crates/holochain_state/CHANGELOG.md#0.2.5-rc.0)

## [hdi-0.3.5-rc.0](crates/hdi/CHANGELOG.md#0.3.5-rc.0)

## [holochain\_p2p-0.2.5-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.5-rc.0)

## [hdk\_derive-0.2.5-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_types-0.2.5-rc.0](crates/holochain_types/CHANGELOG.md#0.2.5-rc.0)

Refactor: Wasmer-related items from holochain\_types have been moved to crate `holochain_wasmer_host::module`.

## [holochain\_keystore-0.2.5-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_sqlite-0.2.5-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.5-rc.0)

- Provide a mechanism to automatically encrypt databases which are currently unencrypted. This is useful if you are switching from a Holochain built with the `sqlite` feature, to a Holochain built with `sqlite-encrypted`. In order to enable this mechanism you will need to set the environment variable `HOLOCHAIN_MIGRATE_UNENCRYPTED=true`. *DANGER*: If you switch your Holochain without this environment variable then on first startup it will recognise your cache, dht, peer and kitsune metrics databases will be recognised as corrupt and automatically wiped. These databases may be rebuilt, assuming that the same data is still available from other peers, but please consider making a backup before attempting to make the switch.

## [holochain\_zome\_types-0.2.5-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p-0.2.5-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.5-rc.0)

## [holochain\_integrity\_types-0.2.5-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_fetch-0.2.5-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_proxy-0.2.5-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.5-rc.0)

## [holo\_hash-0.2.5-rc.0](crates/holo_hash/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.5-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.5-rc.0)

## [fixt-0.2.5-rc.0](crates/fixt/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_types-0.2.5-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_block-0.2.5-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_dht-0.2.5-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.5-rc.0)

## [kitsune\_p2p\_timestamp-0.2.5-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.5-rc.0)

# 20231214.142611

## [holochain\_cli-0.2.4](crates/holochain_cli/CHANGELOG.md#0.2.4)

## [holochain-0.2.4](crates/holochain/CHANGELOG.md#0.2.4)

## [holochain\_cli\_bundle-0.2.4](crates/holochain_cli_bundle/CHANGELOG.md#0.2.4)

## [holochain\_cli\_run\_local\_services-0.2.4](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.4)

## [holochain\_cli\_sandbox-0.2.4](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.4)

## [holochain\_cascade-0.2.4](crates/holochain_cascade/CHANGELOG.md#0.2.4)

## [holochain\_conductor\_api-0.2.4](crates/holochain_conductor_api/CHANGELOG.md#0.2.4)

## [holochain\_metrics-0.2.4](crates/holochain_metrics/CHANGELOG.md#0.2.4)

## [holochain\_test\_wasm\_common-0.2.4](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.4)

## [holochain\_wasm\_test\_utils-0.2.4](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.4)

## [holochain\_websocket-0.2.4](crates/holochain_websocket/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_bootstrap-0.1.4](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.4)

## [hdk-0.2.4](crates/hdk/CHANGELOG.md#0.2.4)

## [holochain\_state-0.2.4](crates/holochain_state/CHANGELOG.md#0.2.4)

## [hdi-0.3.4](crates/hdi/CHANGELOG.md#0.3.4)

## [holochain\_p2p-0.2.4](crates/holochain_p2p/CHANGELOG.md#0.2.4)

## [hdk\_derive-0.2.4](crates/hdk_derive/CHANGELOG.md#0.2.4)

## [holochain\_types-0.2.4](crates/holochain_types/CHANGELOG.md#0.2.4)

## [holochain\_keystore-0.2.4](crates/holochain_keystore/CHANGELOG.md#0.2.4)

## [holochain\_sqlite-0.2.4](crates/holochain_sqlite/CHANGELOG.md#0.2.4)

## [mr\_bundle-0.2.4](crates/mr_bundle/CHANGELOG.md#0.2.4)

## [holochain\_zome\_types-0.2.4](crates/holochain_zome_types/CHANGELOG.md#0.2.4)

## [kitsune\_p2p-0.2.4](crates/kitsune_p2p/CHANGELOG.md#0.2.4)

## [holochain\_integrity\_types-0.2.4](crates/holochain_integrity_types/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_fetch-0.2.4](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_mdns-0.2.4](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_proxy-0.2.4](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.4)

## [holo\_hash-0.2.4](crates/holo_hash/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_transport\_quic-0.2.4](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.4)

## [fixt-0.2.4](crates/fixt/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_types-0.2.4](crates/kitsune_p2p_types/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_block-0.2.4](crates/kitsune_p2p_block/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_dht-0.2.4](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_bin\_data-0.2.4](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_timestamp-0.2.4](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.4)

## [holochain\_util-0.2.4](crates/holochain_util/CHANGELOG.md#0.2.4)

## [kitsune\_p2p\_dht\_arc-0.2.4](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.4)

## [holochain\_trace-0.2.4](crates/holochain_trace/CHANGELOG.md#0.2.4)

# 20231211.130935

## [holochain\_cli-0.2.4-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.4-rc.0)

## [holochain-0.2.4-rc.0](crates/holochain/CHANGELOG.md#0.2.4-rc.0)

- Fix an issue where app validation for StoreRecord ops with a Delete or DeleteLink action were always passed to all zomes. These ops are now only passed to the zome which defined the entry type of the op that is being deleted. [\#3107](https://github.com/holochain/holochain/pull/3107)

- Refactor: Remove shadowing glob re-exports that were shadowing other exports.

- Fix: Countersigning test `lock_chain` which ensures that source chain is locked while in a countersigning session.

- Major refactor of the sys validation workflow to improve reliability and performance:
  
  - Reliability: The workflow will now prioritise validating ops that have their dependencies available locally. As soon as it has finished with those it will trigger app validation before dealing with missing dependencies.
  - Reliability: For ops which have dependencies we aren’t holding locally, the network get will now be retried. This was a cause of undesirable behaviour for validation where a failed get would result in validation for ops with missing dependencies not being retried until new ops arrived. The workflow now retries the get on an interval until it finds dependencies and can proceed with validation.
  - Performance and correctness: A feature which captured and processed ops that were discovered during validation has been removed. This had been added as an attempt to avoid deadlocks within validation but if that happens there’s a bug somewhere else. Sys validation needs to trust that Holochain will correctly manage its current arc and that we will get that data eventually through publishing or gossip. This probably wasn’t doing a lot of harm but it was uneccessary and doing database queries so it should be good to have that gone.
  - Performance: In-memory caching for sys validation dependencies. When we have to wait to validate an op because it has a missing dependency, any other actions required by that op will be held in memory rather than being refetched from the database. This has a fairly small memory footprint because actions are relatively small but saves repeatedly hitting the cascade for the same data if it takes a bit of time to find a dependency on the network.

- Added a new check to system validation to ensure that the `original_entry_address` of an update points to the same entry hash that the original action pointed to. [3023](https://github.com/holochain/holochain/pull/3023)

- Fix: App validation would not be retriggered for ops that failed validation. The app validation workflow had only been retriggered when the number of concurrent ops to be validated (50) was reached. Now the workflow will be re-triggered after a 10 second delay whenever any ops could not be validated.

## [holochain\_cli\_bundle-0.2.4-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_cli\_run\_local\_services-0.2.4-rc.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_cli\_sandbox-0.2.4-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_cascade-0.2.4-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_conductor\_api-0.2.4-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_metrics-0.2.4-rc.0](crates/holochain_metrics/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_test\_wasm\_common-0.2.4-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_wasm\_test\_utils-0.2.4-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p\_bootstrap-0.1.4-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.4-rc.0)

## [hdk-0.2.4-rc.0](crates/hdk/CHANGELOG.md#0.2.4-rc.0)

- Remove access to `Timestamp::now()` which comes from `kitsune_p2p_timestamp` and was not supposed to be available in WASM. It would always panic in WASM calls so it should be safe to assume that nobody was actually using this in real apps. If you were trying to and this breaks your hApp then please consider using `sys_time` from the HDK instead which is safe to use for getting the current time.

## [holochain\_state-0.2.4-rc.0](crates/holochain_state/CHANGELOG.md#0.2.4-rc.0)

- Fix: Maximum one unrestricted cap grant was looked up from the source chain to authorize a remote call. Now all unrestricted cap grants are checked for validity.

## [hdi-0.3.4-rc.0](crates/hdi/CHANGELOG.md#0.3.4-rc.0)

## [holochain\_p2p-0.2.4-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.4-rc.0)

## [hdk\_derive-0.2.4-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.4-rc.0)

- Added a macro `#![dna_properties]` which when applied to a struct, exposes a function `try_from_dna_properties()` on that struct

## [holochain\_types-0.2.4-rc.0](crates/holochain_types/CHANGELOG.md#0.2.4-rc.0)

- **BREAKING CHANGE**: A `DnaManifest` and all its sub-fields will now reject unknown fields when deserialized. This will make it harder to provide an invalid DNA manifest to Holochain without realising. For example, coordinator zomes not appearing in your installed hApp because their field was indented to the wrong place. This is not a breaking change for valid manifests but Holochain will now reject more invalid manifests.

## [holochain\_keystore-0.2.4-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_sqlite-0.2.4-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.4-rc.0)

## [holochain\_zome\_types-0.2.4-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p-0.2.4-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.4-rc.0)

- Fixed spammy “Recorded initiate|accept with current round already set” warning. [\#3060](https://github.com/holochain/holochain/pull/3060)

- Gossip send failures and target expired events are now logged as warnings rather than errors, and have additional text for clarity. [\#2974](https://github.com/holochain/holochain/pull/2974)

## [holochain\_integrity\_types-0.2.4-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p\_fetch-0.2.4-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p\_proxy-0.2.4-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.4-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p\_types-0.2.4-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p\_block-0.2.4-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.4-rc.0)

## [kitsune\_p2p\_dht-0.2.4-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.4-rc.0)

# 20231122.134012

## [holochain\_cli-0.2.3](crates/holochain_cli/CHANGELOG.md#0.2.3)

## [holochain\_cli\_sandbox-0.2.3](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.3)

## [holochain\_cli\_run\_local\_services-0.2.3](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.3)

## [holochain\_cli\_bundle-0.2.3](crates/holochain_cli_bundle/CHANGELOG.md#0.2.3)

## [holochain-0.2.3](crates/holochain/CHANGELOG.md#0.2.3)

## [holochain\_websocket-0.2.3](crates/holochain_websocket/CHANGELOG.md#0.2.3)

## [holochain\_test\_wasm\_common-0.2.3](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.3)

## [holochain\_conductor\_api-0.2.3](crates/holochain_conductor_api/CHANGELOG.md#0.2.3)

## [holochain\_wasm\_test\_utils-0.2.3](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.3)

## [holochain\_cascade-0.2.3](crates/holochain_cascade/CHANGELOG.md#0.2.3)

## [holochain\_state-0.2.3](crates/holochain_state/CHANGELOG.md#0.2.3)

## [holochain\_p2p-0.2.3](crates/holochain_p2p/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_bootstrap-0.1.3](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.3)

## [holochain\_types-0.2.3](crates/holochain_types/CHANGELOG.md#0.2.3)

## [holochain\_sqlite-0.2.3](crates/holochain_sqlite/CHANGELOG.md#0.2.3)

## [kitsune\_p2p-0.2.3](crates/kitsune_p2p/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_proxy-0.2.3](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_transport\_quic-0.2.3](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_mdns-0.2.3](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_fetch-0.2.3](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.3)

## [holochain\_keystore-0.2.3](crates/holochain_keystore/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_types-0.2.3](crates/kitsune_p2p_types/CHANGELOG.md#0.2.3)

## [mr\_bundle-0.2.3](crates/mr_bundle/CHANGELOG.md#0.2.3)

## [hdk-0.2.3](crates/hdk/CHANGELOG.md#0.2.3)

## [holochain\_zome\_types-0.2.3](crates/holochain_zome_types/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_block-0.2.3](crates/kitsune_p2p_block/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_bin\_data-0.2.3](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.3)

## [hdi-0.3.3](crates/hdi/CHANGELOG.md#0.3.3)

## [hdk\_derive-0.2.3](crates/hdk_derive/CHANGELOG.md#0.2.3)

## [holochain\_integrity\_types-0.2.3](crates/holochain_integrity_types/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_dht-0.2.3](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_timestamp-0.2.3](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.3)

## [holo\_hash-0.2.3](crates/holo_hash/CHANGELOG.md#0.2.3)

## [kitsune\_p2p\_dht\_arc-0.2.3](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.3)

## [holochain\_trace-0.2.3](crates/holochain_trace/CHANGELOG.md#0.2.3)

## [holochain\_util-0.2.3](crates/holochain_util/CHANGELOG.md#0.2.3)

## [fixt-0.2.3](crates/fixt/CHANGELOG.md#0.2.3)

# 20231115.183324

## [holochain\_cli-0.2.3-rc.1](crates/holochain_cli/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_cli\_sandbox-0.2.3-rc.1](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_cli\_run\_local\_services-0.2.3-rc.1](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_cli\_bundle-0.2.3-rc.1](crates/holochain_cli_bundle/CHANGELOG.md#0.2.3-rc.1)

## [holochain-0.2.3-rc.1](crates/holochain/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_websocket-0.2.3-rc.1](crates/holochain_websocket/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_test\_wasm\_common-0.2.3-rc.1](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_conductor\_api-0.2.3-rc.1](crates/holochain_conductor_api/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_wasm\_test\_utils-0.2.3-rc.1](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_cascade-0.2.3-rc.1](crates/holochain_cascade/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_state-0.2.3-rc.1](crates/holochain_state/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_p2p-0.2.3-rc.1](crates/holochain_p2p/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_bootstrap-0.1.3-rc.1](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.3-rc.1)

## [holochain\_types-0.2.3-rc.1](crates/holochain_types/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_sqlite-0.2.3-rc.1](crates/holochain_sqlite/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p-0.2.3-rc.1](crates/kitsune_p2p/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_proxy-0.2.3-rc.1](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_transport\_quic-0.2.3-rc.1](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_fetch-0.2.3-rc.1](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_keystore-0.2.3-rc.1](crates/holochain_keystore/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_types-0.2.3-rc.1](crates/kitsune_p2p_types/CHANGELOG.md#0.2.3-rc.1)

## [hdk-0.2.3-rc.1](crates/hdk/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_zome\_types-0.2.3-rc.1](crates/holochain_zome_types/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_block-0.2.3-rc.1](crates/kitsune_p2p_block/CHANGELOG.md#0.2.3-rc.1)

## [hdi-0.3.3-rc.1](crates/hdi/CHANGELOG.md#0.3.3-rc.1)

## [hdk\_derive-0.2.3-rc.1](crates/hdk_derive/CHANGELOG.md#0.2.3-rc.1)

## [holochain\_integrity\_types-0.2.3-rc.1](crates/holochain_integrity_types/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_dht-0.2.3-rc.1](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.3-rc.1)

## [kitsune\_p2p\_timestamp-0.2.3-rc.1](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.3-rc.1)

## [holo\_hash-0.2.3-rc.1](crates/holo_hash/CHANGELOG.md#0.2.3-rc.1)

## [fixt-0.2.3-rc.1](crates/fixt/CHANGELOG.md#0.2.3-rc.1)

# 20231108.185648

Bump holonix rust version to 1.71.1. [\#2660](https://github.com/holochain/holochain/pull/2660)

## [holochain\_cli-0.2.3-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_cli\_sandbox-0.2.3-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_cli\_run\_local\_services-0.2.3-rc.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_cli\_bundle-0.2.3-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.3-rc.0)

## [holochain-0.2.3-rc.0](crates/holochain/CHANGELOG.md#0.2.3-rc.0)

- Fix an issue where enough validation receipts being received would not prevent the publish workflow from continuing to run. This was a terrible waste of data and compute and would build up over time as Holochain is used. [2931](https://github.com/holochain/holochain/pull/2931)
- Improve log output for op publishing to accurately reflect the number of ops to be published. The number published which is logged later is accurate and it was confusing to see more ops published than were supposed to be. [2922](https://github.com/holochain/holochain/pull/2922)
- Fix an issue which prevented the publish loop for a cell from suspending if there was either 1. publish activity pending for other cells or 2. enough validation receipts received. [2922](https://github.com/holochain/holochain/pull/2922)
- Fix an issue where receiving incoming ops can accidentally filter out some DHT data until Holochain is restarted. The state management for in-flight DHT ops is now guaranteed by a `Drop` implementation which will clean up state when the `incoming_dht_ops_workflow` finishes. [2913](https://github.com/holochain/holochain/pull/2913)
- Performance improvement when sending validation receipts. When a batch of DHT ops is being processed and an author is unreachable it will no longer spend time trying to send more receipts to that author in serial and instead it sends receipts as a single batch per author. [2848](https://github.com/holochain/holochain/pull/2848)
- Resilience improvement with handling keystore errors in the validation receipt workflow. Previously, all errors caused the workflow to restart from the beginning. This was good for transient errors such as the keystore being unavailable but it also meant that a single validation receipt failing to be signed (e.g. due to a local agent key being removed from the keystore) would prevent any more validation receipts being sent by that conductor. [2848](https://github.com/holochain/holochain/pull/2848)
- **BREAKING CHANGE** Addressed an outstanding technical debt item to make the validation receipt workflow send a network notification (fire and forget) rather than waiting for a response. When the validation receipt workflow was written this functionality wasn’t available but now that it is, sending validation receipts can be sped up by not waiting for a peer to respond. The format has also been changed from sending one receipt at a time to sending batches so it was not possible to maintain backwards compatibility here. [2848](https://github.com/holochain/holochain/pull/2848)

## [holochain\_websocket-0.2.3-rc.0](crates/holochain_websocket/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_test\_wasm\_common-0.2.3-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_conductor\_api-0.2.3-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_wasm\_test\_utils-0.2.3-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_cascade-0.2.3-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_state-0.2.3-rc.0](crates/holochain_state/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_p2p-0.2.3-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_bootstrap-0.1.3-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.3-rc.0)

## [holochain\_types-0.2.3-rc.0](crates/holochain_types/CHANGELOG.md#0.2.3-rc.0)

- In the CloneOnly provisioning strategy, `installed_hash` is no longer required (it’s now optional). [\#2600](https://github.com/holochain/holochain/pull/2600)

## [holochain\_sqlite-0.2.3-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p-0.2.3-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.3-rc.0)

- Add additional configuration options to network\_tuning for setting the allowed ephemeral port range for tx5 connections: tx5\_min\_ephemeral\_udp\_port and tx5\_max\_ephemeral\_udp\_port
- Adjust bootstrap max\_delay from 60 minutes -\> 5 minutes [\#2948](https://github.com/holochain/holochain/pull/2948)

## [kitsune\_p2p\_proxy-0.2.3-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.3-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_mdns-0.2.3-rc.0](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_fetch-0.2.3-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_keystore-0.2.3-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_types-0.2.3-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.3-rc.0)

## [mr\_bundle-0.2.3-rc.0](crates/mr_bundle/CHANGELOG.md#0.2.3-rc.0)

## [hdk-0.2.3-rc.0](crates/hdk/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_zome\_types-0.2.3-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_block-0.2.3-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_bin\_data-0.2.3-rc.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.3-rc.0)

## [hdi-0.3.3-rc.0](crates/hdi/CHANGELOG.md#0.3.3-rc.0)

## [hdk\_derive-0.2.3-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_integrity\_types-0.2.3-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_dht-0.2.3-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_timestamp-0.2.3-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.3-rc.0)

## [holo\_hash-0.2.3-rc.0](crates/holo_hash/CHANGELOG.md#0.2.3-rc.0)

## [kitsune\_p2p\_dht\_arc-0.2.3-rc.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_trace-0.2.3-rc.0](crates/holochain_trace/CHANGELOG.md#0.2.3-rc.0)

## [holochain\_util-0.2.3-rc.0](crates/holochain_util/CHANGELOG.md#0.2.3-rc.0)

## [fixt-0.2.3-rc.0](crates/fixt/CHANGELOG.md#0.2.3-rc.0)

# 20230930.114759

## [holochain\_cli-0.2.3-beta-rc.1](crates/holochain_cli/CHANGELOG.md#0.2.3-beta-rc.1)

## [holochain\_cli\_sandbox-0.2.3-beta-rc.1](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.3-beta-rc.1)

## [holochain-0.2.3-beta-rc.1](crates/holochain/CHANGELOG.md#0.2.3-beta-rc.1)

- Fix: App interfaces are persisted when shutting down conductor. After restart, app interfaces without connected receiver websocket had signal emission fail altogether. Send errors are only logged now instead.

## [holochain\_conductor\_api-0.2.3-beta-rc.1](crates/holochain_conductor_api/CHANGELOG.md#0.2.3-beta-rc.1)

# 20230926.121405

## [holochain\_cli-0.2.3-beta-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_cli\_sandbox-0.2.3-beta-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.3-beta-rc.0)

- `hc sandbox generate` and `hc sandbox run` now exit when the conductor(s) failed to spawn. previously it would wait for the user to cancel manually. [\#2747](https://github.com/holochain/holochain/pull/2747)

## [holochain\_cli\_run\_local\_services-0.2.3-beta-rc.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_cli\_bundle-0.2.3-beta-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain-0.2.3-beta-rc.0](crates/holochain/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_test\_wasm\_common-0.2.3-beta-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_conductor\_api-0.2.3-beta-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_wasm\_test\_utils-0.2.3-beta-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_cascade-0.2.3-beta-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_state-0.2.3-beta-rc.0](crates/holochain_state/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_p2p-0.2.3-beta-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_bootstrap-0.1.3-beta-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.3-beta-rc.0)

## [holochain\_types-0.2.3-beta-rc.0](crates/holochain_types/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_sqlite-0.2.3-beta-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p-0.2.3-beta-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.3-beta-rc.0)

- Resolves several cases where the meta net task would not stop on fatal errors and would not correctly handle other errors [\#2762](https://github.com/holochain/holochain/pull/2762)
- Resolves an issue where a `FetchOp` could skip processing op hashes if getting a topology for the space from the host failed [\#2737](https://github.com/holochain/holochain/pull/2737)
- Adds a warning log if incoming op data pushes are dropped due to a hashing failure on the host [\#2737](https://github.com/holochain/holochain/pull/2737)
- Fixes an issue where sending an unexpected request payload would cause the process to crash [\#2737](https://github.com/holochain/holochain/pull/2737)

## [kitsune\_p2p\_proxy-0.2.3-beta-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.3-beta-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_fetch-0.2.3-beta-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_keystore-0.2.3-beta-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_types-0.2.3-beta-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.3-beta-rc.0)

## [hdk-0.2.3-beta-rc.0](crates/hdk/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_zome\_types-0.2.3-beta-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_block-0.2.3-beta-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_bin\_data-0.2.3-beta-rc.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.3-beta-rc.0)

## [hdi-0.3.3-beta-rc.0](crates/hdi/CHANGELOG.md#0.3.3-beta-rc.0)

## [hdk\_derive-0.2.3-beta-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.3-beta-rc.0)

## [holochain\_integrity\_types-0.2.3-beta-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_dht-0.2.3-beta-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_timestamp-0.2.3-beta-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.3-beta-rc.0)

## [holo\_hash-0.2.3-beta-rc.0](crates/holo_hash/CHANGELOG.md#0.2.3-beta-rc.0)

## [kitsune\_p2p\_dht\_arc-0.2.3-beta-rc.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.3-beta-rc.0)

# 20230913.190722

## [holochain\_cli-0.2.2](crates/holochain_cli/CHANGELOG.md#0.2.2)

## [holochain\_cli\_sandbox-0.2.2](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.2)

## [holochain\_cli\_run\_local\_services-0.2.2](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.2)

## [holochain\_cli\_bundle-0.2.2](crates/holochain_cli_bundle/CHANGELOG.md#0.2.2)

## [holochain-0.2.2](crates/holochain/CHANGELOG.md#0.2.2)

## [holochain\_websocket-0.2.2](crates/holochain_websocket/CHANGELOG.md#0.2.2)

## [holochain\_test\_wasm\_common-0.2.2](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.2)

## [holochain\_conductor\_api-0.2.2](crates/holochain_conductor_api/CHANGELOG.md#0.2.2)

## [holochain\_wasm\_test\_utils-0.2.2](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.2)

## [holochain\_cascade-0.2.2](crates/holochain_cascade/CHANGELOG.md#0.2.2)

## [holochain\_state-0.2.2](crates/holochain_state/CHANGELOG.md#0.2.2)

## [holochain\_p2p-0.2.2](crates/holochain_p2p/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_bootstrap-0.1.2](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.2)

## [holochain\_types-0.2.2](crates/holochain_types/CHANGELOG.md#0.2.2)

## [holochain\_sqlite-0.2.2](crates/holochain_sqlite/CHANGELOG.md#0.2.2)

## [kitsune\_p2p-0.2.2](crates/kitsune_p2p/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_proxy-0.2.2](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_transport\_quic-0.2.2](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_mdns-0.2.2](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_fetch-0.2.2](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.2)

## [holochain\_keystore-0.2.2](crates/holochain_keystore/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_types-0.2.2](crates/kitsune_p2p_types/CHANGELOG.md#0.2.2)

## [mr\_bundle-0.2.2](crates/mr_bundle/CHANGELOG.md#0.2.2)

## [hdk-0.2.2](crates/hdk/CHANGELOG.md#0.2.2)

## [holochain\_zome\_types-0.2.2](crates/holochain_zome_types/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_block-0.2.2](crates/kitsune_p2p_block/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_bin\_data-0.2.2](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.2)

## [hdi-0.3.2](crates/hdi/CHANGELOG.md#0.3.2)

## [hdk\_derive-0.2.2](crates/hdk_derive/CHANGELOG.md#0.2.2)

## [holochain\_integrity\_types-0.2.2](crates/holochain_integrity_types/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_dht-0.2.2](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_timestamp-0.2.2](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.2)

## [holo\_hash-0.2.2](crates/holo_hash/CHANGELOG.md#0.2.2)

## [kitsune\_p2p\_dht\_arc-0.2.2](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.2)

## [holochain\_trace-0.2.2](crates/holochain_trace/CHANGELOG.md#0.2.2)

## [holochain\_util-0.2.2](crates/holochain_util/CHANGELOG.md#0.2.2)

## [fixt-0.2.2](crates/fixt/CHANGELOG.md#0.2.2)

# 20230911.135238

## [holochain\_cli-0.2.2-beta-rc.2](crates/holochain_cli/CHANGELOG.md#0.2.2-beta-rc.2)

- Adds `chc` feature which is recommended if you want to work with a Holochain instance that is built with its `chc` feature. If you are not using CHC you can safely ignore this feature.

## [holochain\_cli\_sandbox-0.2.2-beta-rc.2](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.2-beta-rc.2)

## [holochain\_cli\_bundle-0.2.2-beta-rc.2](crates/holochain_cli_bundle/CHANGELOG.md#0.2.2-beta-rc.2)

## [holochain-0.2.2-beta-rc.3](crates/holochain/CHANGELOG.md#0.2.2-beta-rc.3)

## [holochain\_conductor\_api-0.2.2-beta-rc.2](crates/holochain_conductor_api/CHANGELOG.md#0.2.2-beta-rc.2)

- Adds `ignore_genesis_failure` field to InstallApp arguments. The default is `false`, and can only use this with the CHC feature. [2612](https://github.com/holochain/holochain/pull/2612)

## [holochain\_wasm\_test\_utils-0.2.2-beta-rc.2](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.2-beta-rc.2)

## [holochain\_cascade-0.2.2-beta-rc.2](crates/holochain_cascade/CHANGELOG.md#0.2.2-beta-rc.2)

## [holochain\_state-0.2.2-beta-rc.2](crates/holochain_state/CHANGELOG.md#0.2.2-beta-rc.2)

## [holochain\_p2p-0.2.2-beta-rc.2](crates/holochain_p2p/CHANGELOG.md#0.2.2-beta-rc.2)

## [holochain\_types-0.2.2-beta-rc.2](crates/holochain_types/CHANGELOG.md#0.2.2-beta-rc.2)

# 20230907.154450

## [holochain-0.2.2-beta-rc.2](crates/holochain/CHANGELOG.md#0.2.2-beta-rc.2)

# 20230904.144618

## [holochain\_cli-0.2.2-beta-rc.1](crates/holochain_cli/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_cli\_sandbox-0.2.2-beta-rc.1](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_cli\_run\_local\_services-0.2.2-beta-rc.1](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_cli\_bundle-0.2.2-beta-rc.1](crates/holochain_cli_bundle/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain-0.2.2-beta-rc.1](crates/holochain/CHANGELOG.md#0.2.2-beta-rc.1)

- Change `GenesisFailed` error to include `CellId` so that genesis failures can be correlated with the cells that failed. [2733](https://github.com/holochain/holochain/pull/2733)

## [holochain\_test\_wasm\_common-0.2.2-beta-rc.1](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_conductor\_api-0.2.2-beta-rc.1](crates/holochain_conductor_api/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_wasm\_test\_utils-0.2.2-beta-rc.1](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_cascade-0.2.2-beta-rc.1](crates/holochain_cascade/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_state-0.2.2-beta-rc.1](crates/holochain_state/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_p2p-0.2.2-beta-rc.1](crates/holochain_p2p/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p\_bootstrap-0.1.2-beta-rc.1](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.2-beta-rc.1)

## [holochain\_types-0.2.2-beta-rc.1](crates/holochain_types/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_sqlite-0.2.2-beta-rc.1](crates/holochain_sqlite/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p-0.2.2-beta-rc.1](crates/kitsune_p2p/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p\_proxy-0.2.2-beta-rc.1](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p\_transport\_quic-0.2.2-beta-rc.1](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p\_fetch-0.2.2-beta-rc.1](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_keystore-0.2.2-beta-rc.1](crates/holochain_keystore/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p\_types-0.2.2-beta-rc.1](crates/kitsune_p2p_types/CHANGELOG.md#0.2.2-beta-rc.1)

## [mr\_bundle-0.2.2-beta-rc.0](crates/mr_bundle/CHANGELOG.md#0.2.2-beta-rc.0)

## [hdk-0.2.2-beta-rc.1](crates/hdk/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_zome\_types-0.2.2-beta-rc.1](crates/holochain_zome_types/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p\_block-0.2.2-beta-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.2-beta-rc.0)

## [kitsune\_p2p\_bin\_data-0.2.2-beta-rc.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.2-beta-rc.0)

## [hdi-0.3.2-beta-rc.1](crates/hdi/CHANGELOG.md#0.3.2-beta-rc.1)

## [hdk\_derive-0.2.2-beta-rc.1](crates/hdk_derive/CHANGELOG.md#0.2.2-beta-rc.1)

## [holochain\_integrity\_types-0.2.2-beta-rc.1](crates/holochain_integrity_types/CHANGELOG.md#0.2.2-beta-rc.1)

## [kitsune\_p2p\_dht-0.2.2-beta-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.2-beta-rc.0)

## [kitsune\_p2p\_timestamp-0.2.2-beta-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.2-beta-rc.0)

## [holo\_hash-0.2.2-beta-rc.0](crates/holo_hash/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_util-0.2.2-beta-rc.0](crates/holochain_util/CHANGELOG.md#0.2.2-beta-rc.0)

# 20230830.151554

## [holochain\_cli-0.2.2-beta-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_cli\_sandbox-0.2.2-beta-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_cli\_run\_local\_services-0.2.2-beta-rc.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.2-beta-rc.0)

- Adds function `new()` to `HcRunLocalServices` allowing consumption of `hc_run_local_services` as a lib [\#2705](https://github.com/holochain/holochain/pull/2705)

## [holochain\_cli\_bundle-0.2.2-beta-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.2-beta-rc.0)

- Export packing, unpacking & utility functions of `mr_bundle` in lib exports [\#2705](https://github.com/holochain/holochain/pull/2705)

## [holochain-0.2.2-beta-rc.0](crates/holochain/CHANGELOG.md#0.2.2-beta-rc.0)

- Improves error messages when validation fails with an InvalidCommit error

## [holochain\_test\_wasm\_common-0.2.2-beta-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_conductor\_api-0.2.2-beta-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_wasm\_test\_utils-0.2.2-beta-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_cascade-0.2.2-beta-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_state-0.2.2-beta-rc.0](crates/holochain_state/CHANGELOG.md#0.2.2-beta-rc.0)

- fix: in a scenario where two agents create a cell from the same DNA in the same conductor, cap grant lookup for zome calls succeeded erroneously for any calling agent. The cap grant author was not taken into consideration for the lookup, only the cap secret or the unrestricted cap entry. Fixed by filtering the lookup by cap grant author.

## [holochain\_p2p-0.2.2-beta-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.2-beta-rc.0)

## [kitsune\_p2p\_bootstrap-0.1.2-beta-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.2-beta-rc.0)

## [holochain\_types-0.2.2-beta-rc.0](crates/holochain_types/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_sqlite-0.2.2-beta-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.2-beta-rc.0)

## [kitsune\_p2p-0.2.2-beta-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.2-beta-rc.0)

## [kitsune\_p2p\_proxy-0.2.2-beta-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.2-beta-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.2-beta-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.2-beta-rc.0)

## [kitsune\_p2p\_fetch-0.2.2-beta-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.2-beta-rc.0)

- Fix an issue with merging fetch contexts where merging an item with a context with an item that did not could result in the removal of the context.
- Fix an issue where duplicate fetch sources would be permitted for a single item.

## [holochain\_keystore-0.2.2-beta-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.2-beta-rc.0)

- Update to latest version of Lair keystore. This comes with a minor API change and some dependency changes but is otherwise compatible with the 0.2 series of Lair.

## [kitsune\_p2p\_types-0.2.2-beta-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.2-beta-rc.0)

## [hdk-0.2.2-beta-rc.0](crates/hdk/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_zome\_types-0.2.2-beta-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.2-beta-rc.0)

## [hdi-0.3.2-beta-rc.0](crates/hdi/CHANGELOG.md#0.3.2-beta-rc.0)

## [hdk\_derive-0.2.2-beta-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.2-beta-rc.0)

## [holochain\_integrity\_types-0.2.2-beta-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.2-beta-rc.0)

# 20230724.194148

## [holochain\_cli-0.2.1](crates/holochain_cli/CHANGELOG.md#0.2.1)

## [holochain\_cli\_sandbox-0.2.1](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.1)

## [holochain\_cli\_run\_local\_services-0.2.1](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.1)

## [holochain\_cli\_bundle-0.2.1](crates/holochain_cli_bundle/CHANGELOG.md#0.2.1)

## [holochain-0.2.1](crates/holochain/CHANGELOG.md#0.2.1)

## [holochain\_websocket-0.2.1](crates/holochain_websocket/CHANGELOG.md#0.2.1)

## [holochain\_test\_wasm\_common-0.2.1](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.1)

## [holochain\_conductor\_api-0.2.1](crates/holochain_conductor_api/CHANGELOG.md#0.2.1)

## [holochain\_wasm\_test\_utils-0.2.1](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.1)

## [holochain\_cascade-0.2.1](crates/holochain_cascade/CHANGELOG.md#0.2.1)

## [holochain\_state-0.2.1](crates/holochain_state/CHANGELOG.md#0.2.1)

## [holochain\_p2p-0.2.1](crates/holochain_p2p/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_bootstrap-0.1.1](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.1)

## [holochain\_types-0.2.1](crates/holochain_types/CHANGELOG.md#0.2.1)

## [holochain\_sqlite-0.2.1](crates/holochain_sqlite/CHANGELOG.md#0.2.1)

## [kitsune\_p2p-0.2.1](crates/kitsune_p2p/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_proxy-0.2.1](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_transport\_quic-0.2.1](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_mdns-0.2.1](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_fetch-0.2.1](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.1)

## [holochain\_keystore-0.2.1](crates/holochain_keystore/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_types-0.2.1](crates/kitsune_p2p_types/CHANGELOG.md#0.2.1)

## [mr\_bundle-0.2.1](crates/mr_bundle/CHANGELOG.md#0.2.1)

## [holochain\_util-0.2.1](crates/holochain_util/CHANGELOG.md#0.2.1)

## [hdk-0.2.1](crates/hdk/CHANGELOG.md#0.2.1)

## [holochain\_zome\_types-0.2.1](crates/holochain_zome_types/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_block-0.2.1](crates/kitsune_p2p_block/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_bin\_data-0.2.1](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.1)

## [hdi-0.3.1](crates/hdi/CHANGELOG.md#0.3.1)

## [hdk\_derive-0.2.1](crates/hdk_derive/CHANGELOG.md#0.2.1)

## [holochain\_integrity\_types-0.2.1](crates/holochain_integrity_types/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_dht-0.2.1](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_timestamp-0.2.1](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.1)

## [holo\_hash-0.2.1](crates/holo_hash/CHANGELOG.md#0.2.1)

## [kitsune\_p2p\_dht\_arc-0.2.1](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.1)

## [holochain\_trace-0.2.1](crates/holochain_trace/CHANGELOG.md#0.2.1)

## [fixt-0.2.1](crates/fixt/CHANGELOG.md#0.2.1)

# 20230718.172112

## [holochain\_cli-0.2.1-beta-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.1-beta-rc.0)

- Improved documentation in README, code comments, help text, and error messages.
- Updated from structopt 0.3 to clap 4. [\#2125](https://github.com/holochain/holochain/pull/2125)
- `hc signal-srv` is now `hc run-local-services` and runs both a webrtc signaling server, and the holochain bootstrap server locally. [\#2353](https://github.com/holochain/holochain/pull/2353)

## [holochain\_cli\_sandbox-0.2.1-beta-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.1-beta-rc.0)

- updated comment in src/cli.rs to clarify use of –force-admin-ports
- Improved documentation in README, code comments, help text, and error messages.
- Updated from structopt 0.3 to clap 4. [\#2125](https://github.com/holochain/holochain/pull/2125)
- **BREAKING**: In the course of updates, a bug was discovered which necessitated a breaking change; the short arg for `--holochain-path` used in `hc sandbox` subcommand has changed from `-h` to `-H` to resolve a conflict with the short arg for `--help`. [\#2125](https://github.com/holochain/holochain/pull/2125)

## [holochain\_cli\_run\_local\_services-0.2.1-beta-rc.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.1-beta-rc.0)

- Improved documentation in README, code comments, help text, and error messages.
- Upated from `structopt` 0.3 to `clap` 4.

## [holochain\_cli\_bundle-0.2.1-beta-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.1-beta-rc.0)

- Updated from structopt 0.3 to clap 4. [\#2125](https://github.com/holochain/holochain/pull/2125)

## [holochain-0.2.1-beta-rc.0](crates/holochain/CHANGELOG.md#0.2.1-beta-rc.0)

- Adds experimental Chain Head Coordinator feature, allowing multiple machines to share the same source chain. Holochain must be built with the `chc` feature flag (disabled by default).
- Fixes race condition which caused network instability. Newly joined nodes can get temporarily blocked by other nodes, causing connections to be repeatedly dropped. [\#2534](https://github.com/holochain/holochain/pull/2534)
- **BREAKING CHANGE**: The DhtOp validation rules have been significantly expanded upon, and some logic around what ops are produced when has been altered. Your existing app may experience rejected ops due to these more strict rules.

## [holochain\_websocket-0.2.1-beta-rc.0](crates/holochain_websocket/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_test\_wasm\_common-0.2.1-beta-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_conductor\_api-0.2.1-beta-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.1-beta-rc.0)

- Add links to concepts documentation to the conductor API module.

## [holochain\_wasm\_test\_utils-0.2.1-beta-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_cascade-0.2.1-beta-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_state-0.2.1-beta-rc.0](crates/holochain_state/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_p2p-0.2.1-beta-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_bootstrap-0.1.1-beta-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.1-beta-rc.0)

## [holochain\_types-0.2.1-beta-rc.0](crates/holochain_types/CHANGELOG.md#0.2.1-beta-rc.0)

- **BREAKING CHANGE**: `DhtOp` now uses a `RecordEntry` instead of `Option<Box<Entry>>` to denote the reason why an Entry was not included, if there is no entry included. You can get an `Option<Entry>` via `RecordEntry::as_option()` or `RecordEntry::into_option()`. `Op` and `FlatOp` (used for validation) are unchanged.

## [holochain\_sqlite-0.2.1-beta-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p-0.2.1-beta-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.1-beta-rc.0)

- Fixes bug where authored data cannot be retrieved locally if the storage arc is not covering that data [\#2425](https://github.com/holochain/holochain/pull/2425)
- Add `gossip_arc_clamping` Kitsune tuning param, allowing initial options to set all storage arcs to empty or full. [\#2352](https://github.com/holochain/holochain/pull/2352)
- Changes to arc resizing algorithm to ensure that nodes pick up the slack for freeloading nodes with zero storage arcs. [\#2352](https://github.com/holochain/holochain/pull/2352)
- Disables gossip when using `gossip_arc_clamping = "empty"`: when the arc is clamped to empty, the gossip module doesn’t even activate. [\#2380](https://github.com/holochain/holochain/pull/2380)

## [kitsune\_p2p\_proxy-0.2.1-beta-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.1-beta-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_mdns-0.2.1-beta-rc.0](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_fetch-0.2.1-beta-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_keystore-0.2.1-beta-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_types-0.2.1-beta-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.1-beta-rc.0)

## [mr\_bundle-0.2.1-beta-rc.0](crates/mr_bundle/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_util-0.2.1-beta-rc.0](crates/holochain_util/CHANGELOG.md#0.2.1-beta-rc.0)

## [hdk-0.2.1-beta-rc.0](crates/hdk/CHANGELOG.md#0.2.1-beta-rc.0)

- Add String<TryInto> for Path for easy conversion of Path to string representation
- New v2 of dna info returns full modifiers not just properties. Removed from genesis self check in favour of hdk call. [\#2366](https://github.com/holochain/holochain/pull/2366).
- Add new HDK function `count_links` which accepts a filter that can be applied remotely. This is a more optimal alternative to requesting all links and counting them within a zome function.

## [holochain\_zome\_types-0.2.1-beta-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.1-beta-rc.0)

- Changes the `ChainQueryFilter` to support filtering on multiple entry types and actions types in the same query. The query builder interface hasn’t changed but if your code was calling `entry_type` or `action_type` more than once it will now create a logical OR rather than replacing the action or entry type to filter on.

## [kitsune\_p2p\_block-0.2.1-beta-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_bin\_data-0.2.1-beta-rc.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.1-beta-rc.0)

## [hdi-0.3.1-beta-rc.0](crates/hdi/CHANGELOG.md#0.3.1-beta-rc.0)

## [hdk\_derive-0.2.1-beta-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_integrity\_types-0.2.1-beta-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_dht-0.2.1-beta-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_timestamp-0.2.1-beta-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.1-beta-rc.0)

## [holo\_hash-0.2.1-beta-rc.0](crates/holo_hash/CHANGELOG.md#0.2.1-beta-rc.0)

## [kitsune\_p2p\_dht\_arc-0.2.1-beta-rc.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.1-beta-rc.0)

## [holochain\_trace-0.2.1-beta-rc.0](crates/holochain_trace/CHANGELOG.md#0.2.1-beta-rc.0)

## [fixt-0.2.1-beta-rc.0](crates/fixt/CHANGELOG.md#0.2.1-beta-rc.0)

# 20230515.083847

## [holochain\_cli-0.2.1-beta-dev.0](crates/holochain_cli/CHANGELOG.md#0.2.1-beta-dev.0)

- `hc signal-srv` is now `hc run-local-services` and runs both a webrtc signaling server, and the holochain bootstrap server locally. [\#2353](https://github.com/holochain/holochain/pull/2353)

## [holochain\_cli\_sandbox-0.2.1-beta-dev.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_cli\_run\_local\_services-0.2.1-beta-dev.0](crates/holochain_cli_run_local_services/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_cli\_bundle-0.2.1-beta-dev.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain-0.2.1-beta-dev.0](crates/holochain/CHANGELOG.md#0.2.1-beta-dev.0)

- The feature `test_utils` is no longer a default feature. To consume `sweetest` from this crate please now use `default-features = false` and the feature `sweetest`.

## [holochain\_websocket-0.2.1-beta-dev.0](crates/holochain_websocket/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_test\_wasm\_common-0.2.1-beta-dev.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_conductor\_api-0.2.1-beta-dev.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_wasm\_test\_utils-0.2.1-beta-dev.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_cascade-0.2.1-beta-dev.0](crates/holochain_cascade/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_state-0.2.1-beta-dev.0](crates/holochain_state/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_p2p-0.2.1-beta-dev.0](crates/holochain_p2p/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_bootstrap-0.1.1-beta-dev.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.1-beta-dev.0)

## [holochain\_types-0.2.1-beta-dev.0](crates/holochain_types/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_sqlite-0.2.1-beta-dev.0](crates/holochain_sqlite/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p-0.2.1-beta-dev.0](crates/kitsune_p2p/CHANGELOG.md#0.2.1-beta-dev.0)

- Bump tx5 to include https://github.com/holochain/tx5/pull/31 which should fix the network loop halting on certain error types, like Ban on data send. [\#2315](https://github.com/holochain/holochain/pull/2315)
- Removes the experimental `gossip_single_storage_arc_per_space` tuning param
- Fixes sharded gossip issue where storage arcs are not properly quantized in multi-agent-per-node sharded scenarios. [\#2332](https://github.com/holochain/holochain/pull/2332)

## [kitsune\_p2p\_proxy-0.2.1-beta-dev.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_transport\_quic-0.2.1-beta-dev.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_mdns-0.2.1-beta-dev.0](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_fetch-0.2.1-beta-dev.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_keystore-0.2.1-beta-dev.0](crates/holochain_keystore/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_types-0.2.1-beta-dev.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.1-beta-dev.0)

## [mr\_bundle-0.2.1-beta-dev.0](crates/mr_bundle/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_util-0.2.1-beta-dev.0](crates/holochain_util/CHANGELOG.md#0.2.1-beta-dev.0)

## [hdk-0.2.1-beta-dev.0](crates/hdk/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_zome\_types-0.2.1-beta-dev.0](crates/holochain_zome_types/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_dht-0.2.1-beta-dev.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_block-0.2.1-beta-dev.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_bin\_data-0.2.1-beta-dev.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.1-beta-dev.0)

## [hdi-0.3.1-beta-dev.0](crates/hdi/CHANGELOG.md#0.3.1-beta-dev.0)

## [hdk\_derive-0.2.1-beta-dev.0](crates/hdk_derive/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_integrity\_types-0.2.1-beta-dev.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_timestamp-0.2.1-beta-dev.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.1-beta-dev.0)

## [holo\_hash-0.2.1-beta-dev.0](crates/holo_hash/CHANGELOG.md#0.2.1-beta-dev.0)

## [kitsune\_p2p\_dht\_arc-0.2.1-beta-dev.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.1-beta-dev.0)

## [holochain\_trace-0.2.1-beta-dev.0](crates/holochain_trace/CHANGELOG.md#0.2.1-beta-dev.0)

## [fixt-0.2.1-beta-dev.0](crates/fixt/CHANGELOG.md#0.2.1-beta-dev.0)

# 20230427.171927

This release features a more secure version of Holochain that incorporates advanced network security protocols using WebRTC End-to-End Encryption and addresses known Holochain security vulnerabilities.

It also introduces new functionality and improves some of the existing:

- app-level blocking, for when an application wants to allow agents to block each other for reasons other than validation failures
- user experience improvements that provide better network information
- and improved system validation to make the network more secure and reliable.

## [holochain\_cli-0.2.0](crates/holochain_cli/CHANGELOG.md#0.2.0)

## [holochain\_cli\_signal\_srv-0.2.0](crates/holochain_cli_signal_srv/CHANGELOG.md#0.2.0)

## [holochain\_cli\_sandbox-0.2.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0)

## [holochain\_cli\_bundle-0.2.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0)

## [holochain-0.2.0](crates/holochain/CHANGELOG.md#0.2.0)

## [holochain\_websocket-0.2.0](crates/holochain_websocket/CHANGELOG.md#0.2.0)

## [holochain\_test\_wasm\_common-0.2.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0)

## [holochain\_conductor\_api-0.2.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.0)

## [holochain\_wasm\_test\_utils-0.2.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0)

## [holochain\_cascade-0.2.0](crates/holochain_cascade/CHANGELOG.md#0.2.0)

## [holochain\_state-0.2.0](crates/holochain_state/CHANGELOG.md#0.2.0)

## [holochain\_p2p-0.2.0](crates/holochain_p2p/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_bootstrap-0.1.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.0)

## [holochain\_types-0.2.0](crates/holochain_types/CHANGELOG.md#0.2.0)

## [holochain\_sqlite-0.2.0](crates/holochain_sqlite/CHANGELOG.md#0.2.0)

## [kitsune\_p2p-0.2.0](crates/kitsune_p2p/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_proxy-0.2.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_transport\_quic-0.2.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_mdns-0.2.0](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_fetch-0.2.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.0)

## [holochain\_keystore-0.2.0](crates/holochain_keystore/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_types-0.2.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.0)

## [mr\_bundle-0.2.0](crates/mr_bundle/CHANGELOG.md#0.2.0)

## [holochain\_util-0.2.0](crates/holochain_util/CHANGELOG.md#0.2.0)

## [hdk-0.2.0](crates/hdk/CHANGELOG.md#0.2.0)

## [holochain\_zome\_types-0.2.0](crates/holochain_zome_types/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_dht-0.2.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_block-0.2.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_bin\_data-0.2.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.0)

## [hdi-0.3.0](crates/hdi/CHANGELOG.md#0.3.0)

## [hdk\_derive-0.2.0](crates/hdk_derive/CHANGELOG.md#0.2.0)

## [holochain\_integrity\_types-0.2.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.0)

## [kitsune\_p2p\_timestamp-0.2.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.0)

## [holo\_hash-0.2.0](crates/holo_hash/CHANGELOG.md#0.2.0)

- Adds more ways to convert between different hash types [\#2283](https://github.com/holochain/holochain/pull/2283)
  - Adds `.into_agent_pub_key() -> Option<AgentPubKey>` for `AnyDhtHash` and `AnyLinkableHash`
  - Adds `TryFrom` impls for all fallible conversions. For instance, if you have a link target (of type AnyLinkableHash), you can now do `let entry_hash: EntryHash = link.target.try_into().unwrap()` if you expect the link target to be an entry hash. (Though we don’t recommend using `.unwrap()` in real code\!)

## [kitsune\_p2p\_dht\_arc-0.2.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.0)

## [holochain\_trace-0.2.0](crates/holochain_trace/CHANGELOG.md#0.2.0)

## [fixt-0.2.0](crates/fixt/CHANGELOG.md#0.2.0)

# 20230426.003734

## [holochain\_cli-0.2.0-beta-rc.7](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.7)

- Adds a new `hc signal-srv` command to run a local holochain webrtc signal server that can be passed into a command like `hc sandbox generate network webrtc ws://127.0.0.1:xxx`. [\#2265](https://github.com/holochain/holochain/pull/2265)

## [holochain\_cli\_signal\_srv-0.2.0-beta-rc.2](crates/holochain_cli_signal_srv/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_cli\_sandbox-0.2.0-beta-rc.6](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_cli\_bundle-0.2.0-beta-rc.6](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0-beta-rc.6)

- **Experimental**: `hc dna pack` command now takes `--dylib-ios` option, which produces iOS optimized Zomes. These can be utilized by passing `dylib`: `PathBuf` for Zome configurations in `dna.yaml` files and other data structures based on ZomeManifest where Zomes are constructed. [\#2218](https://github.com/holochain/holochain/pull/2218)

## [holochain-0.2.0-beta-rc.7](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.7)

## [holochain\_websocket-0.2.0-beta-rc.3](crates/holochain_websocket/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_test\_wasm\_common-0.2.0-beta-rc.6](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_conductor\_api-0.2.0-beta-rc.7](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.7)

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.7](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.7)

## [holochain\_cascade-0.2.0-beta-rc.7](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.7)

## [holochain\_state-0.2.0-beta-rc.7](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.7)

## [holochain\_p2p-0.2.0-beta-rc.7](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.7)

## [kitsune\_p2p\_bootstrap-0.1.0-beta-rc.4](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.0-beta-rc.4)

## [holochain\_types-0.2.0-beta-rc.7](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.7)

- `ZomeManifest` now takes a `dylib` argument, with the type `Option<PathBuf>`. It can be safely ignored in cases other than trying to execute on native iOS. It is used with artifacts produced by `hc dna pack` when it has been called with the `--dylib-ios` option. [\#2218](https://github.com/holochain/holochain/pull/2218)

## [holochain\_sqlite-0.2.0-beta-rc.6](crates/holochain_sqlite/CHANGELOG.md#0.2.0-beta-rc.6)

## [kitsune\_p2p-0.2.0-beta-rc.6](crates/kitsune_p2p/CHANGELOG.md#0.2.0-beta-rc.6)

## [kitsune\_p2p\_proxy-0.2.0-beta-rc.5](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p\_transport\_quic-0.2.0-beta-rc.5](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p\_mdns-0.2.0-beta-rc.1](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_fetch-0.2.0-beta-rc.5](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_keystore-0.2.0-beta-rc.6](crates/holochain_keystore/CHANGELOG.md#0.2.0-beta-rc.6)

## [kitsune\_p2p\_types-0.2.0-beta-rc.5](crates/kitsune_p2p_types/CHANGELOG.md#0.2.0-beta-rc.5)

## [mr\_bundle-0.2.0-beta-rc.1](crates/mr_bundle/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_util-0.2.0-beta-rc.1](crates/holochain_util/CHANGELOG.md#0.2.0-beta-rc.1)

## [hdk-0.2.0-beta-rc.6](crates/hdk/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_zome\_types-0.2.0-beta-rc.6](crates/holochain_zome_types/CHANGELOG.md#0.2.0-beta-rc.6)

## [kitsune\_p2p\_dht-0.2.0-beta-rc.3](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_block-0.2.0-beta-rc.4](crates/kitsune_p2p_block/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p\_bin\_data-0.2.0-beta-rc.3](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.0-beta-rc.3)

## [hdi-0.3.0-beta-rc.5](crates/hdi/CHANGELOG.md#0.3.0-beta-rc.5)

## [hdk\_derive-0.2.0-beta-rc.5](crates/hdk_derive/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_integrity\_types-0.2.0-beta-rc.5](crates/holochain_integrity_types/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p\_timestamp-0.2.0-beta-rc.2](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.0-beta-rc.2)

## [holo\_hash-0.2.0-beta-rc.5](crates/holo_hash/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p\_dht\_arc-0.2.0-beta-rc.3](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_trace-0.2.0-beta-rc.2](crates/holochain_trace/CHANGELOG.md#0.2.0-beta-rc.2)

# 20230420.162535

## [holochain\_cli-0.2.0-beta-rc.6](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_cli\_sandbox-0.2.0-beta-rc.5](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0-beta-rc.5)

- Add new option `in-process-lair` to `hc sandbox generate` which causes the generated conductor config to specify an in-process lair. This comes with an associated change to make `hc sandbox run` respect the conductor configuration and only launch a lair instance when required.

## [holochain\_cli\_bundle-0.2.0-beta-rc.5](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain-0.2.0-beta-rc.6](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.6)

- Feature renaming from `no-deps` to `sqlite` and `db-encryption` to `sqlite-encrypted`. It should not be necessary to configure these unless you are packaging `holochain` or have imported it as a dependency without default features. In the latter case, please update any references to the old feature names.

## [holochain\_websocket-0.2.0-beta-rc.2](crates/holochain_websocket/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_test\_wasm\_common-0.2.0-beta-rc.5](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_conductor\_api-0.2.0-beta-rc.6](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.6](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_cascade-0.2.0-beta-rc.6](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_state-0.2.0-beta-rc.6](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_p2p-0.2.0-beta-rc.6](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.6)

## [kitsune\_p2p\_bootstrap-0.1.0-beta-rc.3](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_types-0.2.0-beta-rc.6](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.6)

## [holochain\_sqlite-0.2.0-beta-rc.5](crates/holochain_sqlite/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p-0.2.0-beta-rc.5](crates/kitsune_p2p/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p\_proxy-0.2.0-beta-rc.4](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p\_transport\_quic-0.2.0-beta-rc.4](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p\_fetch-0.2.0-beta-rc.4](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_keystore-0.2.0-beta-rc.5](crates/holochain_keystore/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p\_types-0.2.0-beta-rc.4](crates/kitsune_p2p_types/CHANGELOG.md#0.2.0-beta-rc.4)

## [hdk-0.2.0-beta-rc.5](crates/hdk/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_zome\_types-0.2.0-beta-rc.5](crates/holochain_zome_types/CHANGELOG.md#0.2.0-beta-rc.5)

## [kitsune\_p2p\_dht-0.2.0-beta-rc.2](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.0-beta-rc.2)

## [kitsune\_p2p\_block-0.2.0-beta-rc.3](crates/kitsune_p2p_block/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_bin\_data-0.2.0-beta-rc.2](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.0-beta-rc.2)

## [hdi-0.3.0-beta-rc.4](crates/hdi/CHANGELOG.md#0.3.0-beta-rc.4)

## [hdk\_derive-0.2.0-beta-rc.4](crates/hdk_derive/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_integrity\_types-0.2.0-beta-rc.4](crates/holochain_integrity_types/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p\_timestamp-0.2.0-beta-rc.1](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.0-beta-rc.1)

## [holo\_hash-0.2.0-beta-rc.4](crates/holo_hash/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p\_dht\_arc-0.2.0-beta-rc.2](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.0-beta-rc.2)

# 20230419.003828

## [holochain\_cli-0.2.0-beta-rc.5](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_cli\_sandbox-0.2.0-beta-rc.4](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_cli\_bundle-0.2.0-beta-rc.4](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain-0.2.0-beta-rc.5](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.5)

- Implements the `clone_only` cell provisioning strategy, desgined for situations where no cell should be installed upon app installation but clones may be created later, via `roles[].provisioning.strategy` in the app manifest [\#2243](https://github.com/holochain/holochain/pull/2243)

## [holochain\_conductor\_api-0.2.0-beta-rc.5](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.5)

- `StorageBlob` is an enum that serialized to camel case named variants. Renames all variants to snake case now.

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.5](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_cascade-0.2.0-beta-rc.5](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_state-0.2.0-beta-rc.5](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_p2p-0.2.0-beta-rc.5](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.5)

## [holochain\_types-0.2.0-beta-rc.5](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.5)

# 20230413.232054

## [holochain\_cli-0.2.0-beta-rc.4](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_cli\_sandbox-0.2.0-beta-rc.3](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_cli\_bundle-0.2.0-beta-rc.3](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain-0.2.0-beta-rc.4](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_test\_wasm\_common-0.2.0-beta-rc.4](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_conductor\_api-0.2.0-beta-rc.4](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.4](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_cascade-0.2.0-beta-rc.4](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_state-0.2.0-beta-rc.4](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_p2p-0.2.0-beta-rc.4](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p\_bootstrap-0.1.0-beta-rc.2](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_types-0.2.0-beta-rc.4](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_keystore-0.2.0-beta-rc.4](crates/holochain_keystore/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_sqlite-0.2.0-beta-rc.4](crates/holochain_sqlite/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p-0.2.0-beta-rc.4](crates/kitsune_p2p/CHANGELOG.md#0.2.0-beta-rc.4)

## [kitsune\_p2p\_proxy-0.2.0-beta-rc.3](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_transport\_quic-0.2.0-beta-rc.3](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_fetch-0.2.0-beta-rc.3](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_types-0.2.0-beta-rc.3](crates/kitsune_p2p_types/CHANGELOG.md#0.2.0-beta-rc.3)

## [hdk-0.2.0-beta-rc.4](crates/hdk/CHANGELOG.md#0.2.0-beta-rc.4)

## [holochain\_zome\_types-0.2.0-beta-rc.4](crates/holochain_zome_types/CHANGELOG.md#0.2.0-beta-rc.4)

# 20230412.003659

- Adds new functionality to the conductor app API which returns disk storage information. It is possible to query for the storage space being used by one app or all apps. The storage used is broken down by cell to make it easier to identify intensive components of apps.

## [holochain\_cli-0.2.0-beta-rc.3](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.3)

- Adds new commands to the `hc` CLI which print out JSON schemas for DNA, hApp and web hApps. Use `hc dna schema`, `hc app schema` and `hc web-app schema` to print schemas which can be saved and used as editing aids in your IDE.

## [holochain\_cli\_sandbox-0.2.0-beta-rc.2](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_cli\_bundle-0.2.0-beta-rc.2](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain-0.2.0-beta-rc.3](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.3)

- BREAKING CHANGE - Removes conductor networking types “Proxy” (“proxy”) and “Quic” (“quic”). Please transition to “WebRTC” (“webrtc”). [\#2208](https://github.com/holochain/holochain/pull/2208)
- Adds `DumpNetworkStats` api to admin websocket [\#2182](https://github.com/holochain/holochain/pull/2182).

## [holochain\_test\_wasm\_common-0.2.0-beta-rc.3](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_conductor\_api-0.2.0-beta-rc.3](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.3)

- Adds new functionality to the conductor admin API which returns disk storage information. The storage used by apps is broken down into blobs which are being used by one or more app.

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.3](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_cascade-0.2.0-beta-rc.3](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_state-0.2.0-beta-rc.3](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_p2p-0.2.0-beta-rc.3](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_bootstrap-0.1.0-beta-rc.1](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_types-0.2.0-beta-rc.3](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_keystore-0.2.0-beta-rc.3](crates/holochain_keystore/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_sqlite-0.2.0-beta-rc.3](crates/holochain_sqlite/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p-0.2.0-beta-rc.3](crates/kitsune_p2p/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_proxy-0.2.0-beta-rc.2](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.0-beta-rc.2)

## [kitsune\_p2p\_transport\_quic-0.2.0-beta-rc.2](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.0-beta-rc.2)

## [kitsune\_p2p\_fetch-0.2.0-beta-rc.2](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.0-beta-rc.2)

## [kitsune\_p2p\_types-0.2.0-beta-rc.2](crates/kitsune_p2p_types/CHANGELOG.md#0.2.0-beta-rc.2)

## [hdk-0.2.0-beta-rc.3](crates/hdk/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_zome\_types-0.2.0-beta-rc.3](crates/holochain_zome_types/CHANGELOG.md#0.2.0-beta-rc.3)

## [kitsune\_p2p\_block-0.2.0-beta-rc.2](crates/kitsune_p2p_block/CHANGELOG.md#0.2.0-beta-rc.2)

## [hdi-0.3.0-beta-rc.3](crates/hdi/CHANGELOG.md#0.3.0-beta-rc.3)

## [hdk\_derive-0.2.0-beta-rc.3](crates/hdk_derive/CHANGELOG.md#0.2.0-beta-rc.3)

## [holochain\_integrity\_types-0.2.0-beta-rc.3](crates/holochain_integrity_types/CHANGELOG.md#0.2.0-beta-rc.3)

## [holo\_hash-0.2.0-beta-rc.3](crates/holo_hash/CHANGELOG.md#0.2.0-beta-rc.3)

- **BREAKING CHANGE**: `HoloHash::retype()` is removed from the public API, and some `From<AnyDhtHash>` and `From<AnyLinkableHash>` impls were removed. Instances of casting one hash type to another must be done via the remaining From impls, or via `into_primitive()`, `into_entry_hash()`, `into_action_hash()`, etc. for converting from a composite hash to a primitive hash. See [holo\_hash::aliases](https://github.com/holochain/holochain/blob/bf242f00f7ef84cd7f09efc9770dc632f0da4310/crates/holo_hash/src/aliases.rs#L49-L140) for a full listing. [\#2191](https://github.com/holochain/holochain/pull/2191)

# 20230405.003224

## [holochain\_cli-0.2.0-beta-rc.2](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_cli\_sandbox-0.2.0-beta-rc.1](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0-beta-rc.1)

- Fix bug in `hc sandbox generate`, where a comma-separated argument passed to the `--directories` option was treated as a single directory name. [\#2080](https://github.com/holochain/holochain/pull/2080)

## [holochain\_cli\_bundle-0.2.0-beta-rc.1](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain-0.2.0-beta-rc.2](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.2)

- Fixes bug where supplying a `network_seed` during an `InstallApp` call does not actually update the network seed for roles whose `provisioning` is set to `None` in the manifest. Now the network seed is correctly updated. [\#2102](https://github.com/holochain/holochain/pull/2102)

## [holochain\_test\_wasm\_common-0.2.0-beta-rc.2](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_conductor\_api-0.2.0-beta-rc.2](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.2](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_cascade-0.2.0-beta-rc.2](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_state-0.2.0-beta-rc.2](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_p2p-0.2.0-beta-rc.2](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_types-0.2.0-beta-rc.2](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.2)

- BREAKING: AppManifest’s `version` now only accepts a single optional DNA hash value, whereas previously it could accept a list of values.
- AppManifest’s `version` is renamed to `installed_hash` (there is still an alias to `version` for limited backward compatibility, so the old field name will still work).

## [holochain\_keystore-0.2.0-beta-rc.2](crates/holochain_keystore/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_sqlite-0.2.0-beta-rc.2](crates/holochain_sqlite/CHANGELOG.md#0.2.0-beta-rc.2)

## [kitsune\_p2p-0.2.0-beta-rc.2](crates/kitsune_p2p/CHANGELOG.md#0.2.0-beta-rc.2)

## [hdk-0.2.0-beta-rc.2](crates/hdk/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_zome\_types-0.2.0-beta-rc.2](crates/holochain_zome_types/CHANGELOG.md#0.2.0-beta-rc.2)

## [hdi-0.3.0-beta-rc.2](crates/hdi/CHANGELOG.md#0.3.0-beta-rc.2)

## [hdk\_derive-0.2.0-beta-rc.2](crates/hdk_derive/CHANGELOG.md#0.2.0-beta-rc.2)

## [holochain\_integrity\_types-0.2.0-beta-rc.2](crates/holochain_integrity_types/CHANGELOG.md#0.2.0-beta-rc.2)

## [holo\_hash-0.2.0-beta-rc.2](crates/holo_hash/CHANGELOG.md#0.2.0-beta-rc.2)

# 20230322.003727

## [holochain\_cli-0.2.0-beta-rc.1](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain-0.2.0-beta-rc.1](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_websocket-0.2.0-beta-rc.1](crates/holochain_websocket/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_test\_wasm\_common-0.2.0-beta-rc.1](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_conductor\_api-0.2.0-beta-rc.1](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.1](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_cascade-0.2.0-beta-rc.1](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_state-0.2.0-beta-rc.1](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.1)

- Optimize capability grant verification during zome calls. This speeds up all remote calls, under which fall calls with a cap secret from clients other than the Launcher. Previously hundreds of calls would slow down response time noticeably because of grant verification. Now thousands of calls (rather thousands of records) won’t affect grant verification by more than a millisecond. [\#2097](https://github.com/holochain/holochain/pull/2097)

## [holochain\_p2p-0.2.0-beta-rc.1](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_bootstrap-0.1.0-beta-rc.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_types-0.2.0-beta-rc.1](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_keystore-0.2.0-beta-rc.1](crates/holochain_keystore/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_sqlite-0.2.0-beta-rc.1](crates/holochain_sqlite/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p-0.2.0-beta-rc.1](crates/kitsune_p2p/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_proxy-0.2.0-beta-rc.1](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_transport\_quic-0.2.0-beta-rc.1](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_fetch-0.2.0-beta-rc.1](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_types-0.2.0-beta-rc.1](crates/kitsune_p2p_types/CHANGELOG.md#0.2.0-beta-rc.1)

## [hdk-0.2.0-beta-rc.1](crates/hdk/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_zome\_types-0.2.0-beta-rc.1](crates/holochain_zome_types/CHANGELOG.md#0.2.0-beta-rc.1)

- `name` in DnaDef no longer has an effect on the DNA hash [\#2099](https://github.com/holochain/holochain/pull/2099)

## [kitsune\_p2p\_dht-0.2.0-beta-rc.1](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_block-0.2.0-beta-rc.1](crates/kitsune_p2p_block/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_bin\_data-0.2.0-beta-rc.1](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.0-beta-rc.1)

## [hdi-0.3.0-beta-rc.1](crates/hdi/CHANGELOG.md#0.3.0-beta-rc.1)

## [hdk\_derive-0.2.0-beta-rc.1](crates/hdk_derive/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_integrity\_types-0.2.0-beta-rc.1](crates/holochain_integrity_types/CHANGELOG.md#0.2.0-beta-rc.1)

## [holo\_hash-0.2.0-beta-rc.1](crates/holo_hash/CHANGELOG.md#0.2.0-beta-rc.1)

## [kitsune\_p2p\_dht\_arc-0.2.0-beta-rc.1](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.0-beta-rc.1)

## [holochain\_trace-0.2.0-beta-rc.1](crates/holochain_trace/CHANGELOG.md#0.2.0-beta-rc.1)

# 20230315.183209

## [holochain\_cli-0.2.0-beta-rc.0](crates/holochain_cli/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_cli\_sandbox-0.2.0-beta-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_cli\_bundle-0.2.0-beta-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain-0.2.0-beta-rc.0](crates/holochain/CHANGELOG.md#0.2.0-beta-rc.0)

- When uninstalling an app, local data is now cleaned up where appropriate. [\#1805](https://github.com/holochain/holochain/pull/1805)
  - Detail: any time an app is uninstalled, if the removal of that app’s cells would cause there to be no cell installed which uses a given DNA, the databases for that DNA space are deleted. So, if you have an app installed twice under two different agents and uninstall one of them, no data will be removed, but if you uninstall both, then all local data will be cleaned up. If any of your data was gossiped to other peers though, it will live on in the DHT, and even be gossiped back to you if you reinstall that same app with a new agent.
- Renames `OpType` to `FlatOp`, and `Op::to_type()` to `Op::flattened()`. Aliases for the old names still exist, so this is not a breaking change. [\#1909](https://github.com/holochain/holochain/pull/1909)
- Fixed a [problem with validation of Ops with private entry data](https://github.com/holochain/holochain/issues/1861), where  `Op::to_type()` would fail for private `StoreEntry` ops. [\#1910](https://github.com/holochain/holochain/pull/1910)

## [holochain\_websocket-0.2.0-beta-rc.0](crates/holochain_websocket/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_test\_wasm\_common-0.2.0-beta-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_conductor\_api-0.2.0-beta-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.2.0-beta-rc.0)

- Reject creation of duplicate clone cells. It was possible to create a clone cell with a DNA hash identical to an already existing DNA. [\#1997](https://github.com/holochain/holochain/pull/1997)
- Adds doc comments for `StemCell`, `ProvisionedCell` and `CloneCell` structs

## [holochain\_wasm\_test\_utils-0.2.0-beta-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_cascade-0.2.0-beta-rc.0](crates/holochain_cascade/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_state-0.2.0-beta-rc.0](crates/holochain_state/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_p2p-0.2.0-beta-rc.0](crates/holochain_p2p/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_bootstrap-0.0.12-dev.0](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.12-dev.0)

## [holochain\_types-0.2.0-beta-rc.0](crates/holochain_types/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_keystore-0.2.0-beta-rc.0](crates/holochain_keystore/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_sqlite-0.2.0-beta-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p-0.2.0-beta-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.2.0-beta-rc.0)

- Adds feature flipper `tx5` which enables experimental integration with holochains WebRTC networking backend. This is not enabled by default. [\#1741](https://github.com/holochain/holochain/pull/1741)

## [kitsune\_p2p\_proxy-0.2.0-beta-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_transport\_quic-0.2.0-beta-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_mdns-0.2.0-beta-rc.0](crates/kitsune_p2p_mdns/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_fetch-0.2.0-beta-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_types-0.2.0-beta-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.2.0-beta-rc.0)

## [mr\_bundle-0.2.0-beta-rc.0](crates/mr_bundle/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_util-0.2.0-beta-rc.0](crates/holochain_util/CHANGELOG.md#0.2.0-beta-rc.0)

## [hdk-0.2.0-beta-rc.0](crates/hdk/CHANGELOG.md#0.2.0-beta-rc.0)

- Add block/unblock agent functions to HDK [\#1828](https://github.com/holochain/holochain/pull/1828)

## [holochain\_zome\_types-0.2.0-beta-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_dht-0.2.0-beta-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_block-0.2.0-beta-rc.0](crates/kitsune_p2p_block/CHANGELOG.md#0.2.0-beta-rc.0)

- Crate now exists

## [kitsune\_p2p\_bin\_data-0.2.0-beta-rc.0](crates/kitsune_p2p_bin_data/CHANGELOG.md#0.2.0-beta-rc.0)

- Crate now exists

## [hdi-0.3.0-beta-rc.0](crates/hdi/CHANGELOG.md#0.3.0-beta-rc.0)

## [hdk\_derive-0.2.0-beta-rc.0](crates/hdk_derive/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_integrity\_types-0.2.0-beta-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_timestamp-0.2.0-beta-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.2.0-beta-rc.0)

## [holo\_hash-0.2.0-beta-rc.0](crates/holo_hash/CHANGELOG.md#0.2.0-beta-rc.0)

## [kitsune\_p2p\_dht\_arc-0.2.0-beta-rc.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.2.0-beta-rc.0)

## [holochain\_trace-0.2.0-beta-rc.0](crates/holochain_trace/CHANGELOG.md#0.2.0-beta-rc.0)

## [fixt-0.2.0-beta-rc.0](crates/fixt/CHANGELOG.md#0.2.0-beta-rc.0)

# 20230126.223635

- First beta release

## [holochain\_cli-0.1.0](crates/holochain_cli/CHANGELOG.md#0.1.0)

## [holochain\_cli\_sandbox-0.1.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.1.0)

## [holochain\_cli\_bundle-0.1.0](crates/holochain_cli_bundle/CHANGELOG.md#0.1.0)

## [holochain-0.1.0](crates/holochain/CHANGELOG.md#0.1.0)

## [holochain\_websocket-0.1.0](crates/holochain_websocket/CHANGELOG.md#0.1.0)

## [holochain\_test\_wasm\_common-0.1.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.1.0)

## [holochain\_conductor\_api-0.1.0](crates/holochain_conductor_api/CHANGELOG.md#0.1.0)

## [holochain\_wasm\_test\_utils-0.1.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.1.0)

## [holochain\_cascade-0.1.0](crates/holochain_cascade/CHANGELOG.md#0.1.0)

## [holochain\_state-0.1.0](crates/holochain_state/CHANGELOG.md#0.1.0)

## [holochain\_p2p-0.1.0](crates/holochain_p2p/CHANGELOG.md#0.1.0)

## [holochain\_types-0.1.0](crates/holochain_types/CHANGELOG.md#0.1.0)

## [holochain\_keystore-0.1.0](crates/holochain_keystore/CHANGELOG.md#0.1.0)

## [holochain\_sqlite-0.1.0](crates/holochain_sqlite/CHANGELOG.md#0.1.0)

## [kitsune\_p2p-0.1.0](crates/kitsune_p2p/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_proxy-0.1.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_transport\_quic-0.1.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_mdns-0.1.0](crates/kitsune_p2p_mdns/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_fetch-0.1.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_types-0.1.0](crates/kitsune_p2p_types/CHANGELOG.md#0.1.0)

## [mr\_bundle-0.1.0](crates/mr_bundle/CHANGELOG.md#0.1.0)

## [holochain\_util-0.1.0](crates/holochain_util/CHANGELOG.md#0.1.0)

## [hdk-0.1.0](crates/hdk/CHANGELOG.md#0.1.0)

- Add note in HDK documentation about links not deduplicating. ([\#1791](https://github.com/holochain/holochain/pull/1791))

## [holochain\_zome\_types-0.1.0](crates/holochain_zome_types/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_dht-0.1.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.1.0)

## [hdi-0.2.0](crates/hdi/CHANGELOG.md#0.2.0)

## [hdk\_derive-0.1.0](crates/hdk_derive/CHANGELOG.md#0.1.0)

## [holochain\_integrity\_types-0.1.0](crates/holochain_integrity_types/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_timestamp-0.1.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.1.0)

## [holo\_hash-0.1.0](crates/holo_hash/CHANGELOG.md#0.1.0)

## [kitsune\_p2p\_dht\_arc-0.1.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.1.0)

## [fixt-0.1.0](crates/fixt/CHANGELOG.md#0.1.0)

# 20230120.225800

## [holochain\_cli-0.1.0-beta-rc.4](crates/holochain_cli/CHANGELOG.md#0.1.0-beta-rc.4)

## [holochain-0.1.0-beta-rc.4](crates/holochain/CHANGELOG.md#0.1.0-beta-rc.4)

- Fix: Disabled clone cells are no longer started when conductor restarts. [\#1775](https://github.com/holochain/holochain/pull/1775)

## [holochain\_websocket-0.1.0-beta-rc.1](crates/holochain_websocket/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_test\_wasm\_common-0.1.0-beta-rc.3](crates/holochain_test_wasm_common/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_conductor\_api-0.1.0-beta-rc.4](crates/holochain_conductor_api/CHANGELOG.md#0.1.0-beta-rc.4)

- **BREAKING CHANGE**: `CreateCloneCell` returns `ClonedCell` instead of `InstalledCell`.
- **BREAKING CHANGE**: `EnableCloneCell` returns `ClonedCell` instead of `InstalledCell`.
- **BREAKING CHANGE**: Remove unused call `AdminRequest::StartApp`.
- **BREAKING CHANGE**: `Cell` is split up into `ProvisionedCell` and `ClonedCell`.
- **BREAKING CHANGE**: `CellInfo` variants are renamed to snake case during serde.
- Return additional field `agent_pub_key` in `AppInfo`.

## [holochain\_wasm\_test\_utils-0.1.0-beta-rc.3](crates/holochain_wasm_test_utils/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_cascade-0.1.0-beta-rc.3](crates/holochain_cascade/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_state-0.1.0-beta-rc.3](crates/holochain_state/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_p2p-0.1.0-beta-rc.3](crates/holochain_p2p/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_types-0.1.0-beta-rc.3](crates/holochain_types/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_keystore-0.1.0-beta-rc.3](crates/holochain_keystore/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_sqlite-0.1.0-beta-rc.3](crates/holochain_sqlite/CHANGELOG.md#0.1.0-beta-rc.3)

## [kitsune\_p2p-0.1.0-beta-rc.2](crates/kitsune_p2p/CHANGELOG.md#0.1.0-beta-rc.2)

## [kitsune\_p2p\_proxy-0.1.0-beta-rc.2](crates/kitsune_p2p_proxy/CHANGELOG.md#0.1.0-beta-rc.2)

## [kitsune\_p2p\_transport\_quic-0.1.0-beta-rc.2](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.1.0-beta-rc.2)

## [kitsune\_p2p\_fetch-0.1.0-beta-rc.1](crates/kitsune_p2p_fetch/CHANGELOG.md#0.1.0-beta-rc.1)

## [kitsune\_p2p\_types-0.1.0-beta-rc.2](crates/kitsune_p2p_types/CHANGELOG.md#0.1.0-beta-rc.2)

## [mr\_bundle-0.1.0-beta-rc.2](crates/mr_bundle/CHANGELOG.md#0.1.0-beta-rc.2)

- **BREAKING CHANGE:** The `resources` field of bundles was not properly set up for efficient serialization. Bundles built before this change must now be rebuilt. [\#1723](https://github.com/holochain/holochain/pull/1723)
  - Where the actual bytes of the resource were previously specified by a simple sequence of numbers, now a byte array is expected. For instance, in JavaScript, this is the difference between an Array and a Buffer.

## [hdk-0.1.0-beta-rc.3](crates/hdk/CHANGELOG.md#0.1.0-beta-rc.3)

- Fix typos and links in docs and add links to wasm examples.

## [holochain\_zome\_types-0.1.0-beta-rc.3](crates/holochain_zome_types/CHANGELOG.md#0.1.0-beta-rc.3)

- Added the `author` field to the `Link` struct for easy access after a `get_links` call.

## [hdi-0.2.0-beta-rc.3](crates/hdi/CHANGELOG.md#0.2.0-beta-rc.3)

## [hdk\_derive-0.1.0-beta-rc.3](crates/hdk_derive/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_integrity\_types-0.1.0-beta-rc.3](crates/holochain_integrity_types/CHANGELOG.md#0.1.0-beta-rc.3)

## [holo\_hash-0.1.0-beta-rc.2](crates/holo_hash/CHANGELOG.md#0.1.0-beta-rc.2)

# 20230117.165308

## [holochain\_cli-0.1.0-beta-rc.3](crates/holochain_cli/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain-0.1.0-beta-rc.3](crates/holochain/CHANGELOG.md#0.1.0-beta-rc.3)

- Fix: calling `emit_signal` from the `post_commit` callback caused a panic, this is now fixed [\#1749](https://github.com/holochain/holochain/pull/1749)
- Fixes problem where disabling and re-enabling an app causes all of its cells to become unresponsive to any `get*` requests. [\#1744](https://github.com/holochain/holochain/pull/1744)
- Fixes problem where a disabled cell can continue to respond to zome calls and transmit data until the conductor is restarted. [\#1761](https://github.com/holochain/holochain/pull/1761)
- Adds Ctrl+C handling, so that graceful conductor shutdown is possible. [\#1761](https://github.com/holochain/holochain/pull/1761)
- BREAKING CHANGE - Added zome name to the signal emitted when using `emit_signal`.

## [holochain\_test\_wasm\_common-0.1.0-beta-rc.2](crates/holochain_test_wasm_common/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_conductor\_api-0.1.0-beta-rc.3](crates/holochain_conductor_api/CHANGELOG.md#0.1.0-beta-rc.3)

## [holochain\_wasm\_test\_utils-0.1.0-beta-rc.2](crates/holochain_wasm_test_utils/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_cascade-0.1.0-beta-rc.2](crates/holochain_cascade/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_state-0.1.0-beta-rc.2](crates/holochain_state/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_p2p-0.1.0-beta-rc.2](crates/holochain_p2p/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_types-0.1.0-beta-rc.2](crates/holochain_types/CHANGELOG.md#0.1.0-beta-rc.2)

- BREAKING CHANGE - Added zome name to the signal emitted when using `emit_signal`.

## [holochain\_keystore-0.1.0-beta-rc.2](crates/holochain_keystore/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_sqlite-0.1.0-beta-rc.2](crates/holochain_sqlite/CHANGELOG.md#0.1.0-beta-rc.2)

## [kitsune\_p2p-0.1.0-beta-rc.1](crates/kitsune_p2p/CHANGELOG.md#0.1.0-beta-rc.1)

- Fixes some bad logic around leaving spaces, which can cause problems upon rejoining [\#1744](https://github.com/holochain/holochain/pull/1744)
  - When an agent leaves a space, an `AgentInfoSigned` with an empty arc is published before leaving. Previously, this empty-arc agent info was also persisted to the database, but this is inappropriate because upon rejoining, they will start with an empty arc. Now, the agent info is removed from the database altogether upon leaving.

## [kitsune\_p2p\_proxy-0.1.0-beta-rc.1](crates/kitsune_p2p_proxy/CHANGELOG.md#0.1.0-beta-rc.1)

## [kitsune\_p2p\_transport\_quic-0.1.0-beta-rc.1](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.1.0-beta-rc.1)

## [kitsune\_p2p\_fetch-0.1.0-beta-rc.0](crates/kitsune_p2p_fetch/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p\_types-0.1.0-beta-rc.1](crates/kitsune_p2p_types/CHANGELOG.md#0.1.0-beta-rc.1)

## [mr\_bundle-0.1.0-beta-rc.1](crates/mr_bundle/CHANGELOG.md#0.1.0-beta-rc.1)

## [hdk-0.1.0-beta-rc.2](crates/hdk/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_zome\_types-0.1.0-beta-rc.2](crates/holochain_zome_types/CHANGELOG.md#0.1.0-beta-rc.2)

## [kitsune\_p2p\_dht-0.1.0-beta-rc.1](crates/kitsune_p2p_dht/CHANGELOG.md#0.1.0-beta-rc.1)

## [hdi-0.2.0-beta-rc.2](crates/hdi/CHANGELOG.md#0.2.0-beta-rc.2)

## [hdk\_derive-0.1.0-beta-rc.2](crates/hdk_derive/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_integrity\_types-0.1.0-beta-rc.2](crates/holochain_integrity_types/CHANGELOG.md#0.1.0-beta-rc.2)

## [kitsune\_p2p\_timestamp-0.1.0-beta-rc.1](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.1.0-beta-rc.1)

## [holo\_hash-0.1.0-beta-rc.1](crates/holo_hash/CHANGELOG.md#0.1.0-beta-rc.1)

## [kitsune\_p2p\_dht\_arc-0.1.0-beta-rc.1](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.1.0-beta-rc.1)

## [fixt-0.1.0-beta-rc.1](crates/fixt/CHANGELOG.md#0.1.0-beta-rc.1)

# 20221223.034701

## [holochain\_cli-0.1.0-beta-rc.2](crates/holochain_cli/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain-0.1.0-beta-rc.2](crates/holochain/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_test\_wasm\_common-0.1.0-beta-rc.1](crates/holochain_test_wasm_common/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_conductor\_api-0.1.0-beta-rc.2](crates/holochain_conductor_api/CHANGELOG.md#0.1.0-beta-rc.2)

## [holochain\_wasm\_test\_utils-0.1.0-beta-rc.1](crates/holochain_wasm_test_utils/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_cascade-0.1.0-beta-rc.1](crates/holochain_cascade/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_state-0.1.0-beta-rc.1](crates/holochain_state/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_p2p-0.1.0-beta-rc.1](crates/holochain_p2p/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_types-0.1.0-beta-rc.1](crates/holochain_types/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_keystore-0.1.0-beta-rc.1](crates/holochain_keystore/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_sqlite-0.1.0-beta-rc.1](crates/holochain_sqlite/CHANGELOG.md#0.1.0-beta-rc.1)

## [hdk-0.1.0-beta-rc.1](crates/hdk/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_zome\_types-0.1.0-beta-rc.1](crates/holochain_zome_types/CHANGELOG.md#0.1.0-beta-rc.1)

## [hdi-0.2.0-beta-rc.1](crates/hdi/CHANGELOG.md#0.2.0-beta-rc.1)

## [hdk\_derive-0.1.0-beta-rc.1](crates/hdk_derive/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_integrity\_types-0.1.0-beta-rc.1](crates/holochain_integrity_types/CHANGELOG.md#0.1.0-beta-rc.1)

- **BREAKING CHANGE**: Updated capability grant structure `GrantedFunctions` to be an enum with `All` for allowing all zomes all functions to be called, along with `Listed` to specify a zome and function as before. [\#1732](https://github.com/holochain/holochain/pull/1732)

# 20221216.210935

## [holochain\_cli-0.1.0-beta-rc.1](crates/holochain_cli/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain-0.1.0-beta-rc.1](crates/holochain/CHANGELOG.md#0.1.0-beta-rc.1)

## [holochain\_conductor\_api-0.1.0-beta-rc.1](crates/holochain_conductor_api/CHANGELOG.md#0.1.0-beta-rc.1)

- Fix error while installing app and return app info of newly installed app. [\#1725](https://github.com/holochain/holochain/pull/1725)

# 20221215.173657

- Breaking: Improves Op::to\_type helper to make action info more ergonomic in validation

## [holochain\_cli-0.1.0-beta-rc.0](crates/holochain_cli/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_cli\_sandbox-0.1.0-beta-rc.0](crates/holochain_cli_sandbox/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_cli\_bundle-0.1.0-beta-rc.0](crates/holochain_cli_bundle/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain-0.1.0-beta-rc.0](crates/holochain/CHANGELOG.md#0.1.0-beta-rc.0)

- All zome calls must now be signed by the provenance, the signature is of the hash of the unsigned zome call, a unique nonce and expiry is also required [1510](https://github.com/holochain/holochain/pull/1510/files)

## [holochain\_websocket-0.1.0-beta-rc.0](crates/holochain_websocket/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_test\_wasm\_common-0.1.0-beta-rc.0](crates/holochain_test_wasm_common/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_conductor\_api-0.1.0-beta-rc.0](crates/holochain_conductor_api/CHANGELOG.md#0.1.0-beta-rc.0)

- **BREAKING CHANGE**: Remove deprecated Admin and App API calls.

- **BREAKING CHANGE**: Remove call `InstallApp`.

- **BREAKING CHANGE**: Rename `InstallAppBundle` to `InstallApp`.

- **BREAKING CHANGE**: Rename `ZomeCall` to `CallZome`. [\#1707](https://github.com/holochain/holochain/pull/1707)

- **BREAKING CHANGE**: Rename ArchiveCloneCell to DisableCloneCell.

- **BREAKING CHANGE**: Rename RestoreArchivedCloneCell to EnableCloneCell.

- **BREAKING CHANGE**: Move EnableCloneCell to App API.

- **BREAKING CHANGE**: Refactor DeleteCloneCell to delete a single disabled clone cell. [\#1704](https://github.com/holochain/holochain/pull/1704)

- **BREAKING CHANGE**: Refactor `AppInfo` to return all cells and DNA modifiers.

- **BREAKING CHANGE**: Rename `RequestAgentInfo` to `AgentInfo`. [\#1719](https://github.com/holochain/holochain/pull/1719)

## [holochain\_wasm\_test\_utils-0.1.0-beta-rc.0](crates/holochain_wasm_test_utils/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_cascade-0.1.0-beta-rc.0](crates/holochain_cascade/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_state-0.1.0-beta-rc.0](crates/holochain_state/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_p2p-0.1.0-beta-rc.0](crates/holochain_p2p/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_types-0.1.0-beta-rc.0](crates/holochain_types/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_keystore-0.1.0-beta-rc.0](crates/holochain_keystore/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_sqlite-0.1.0-beta-rc.0](crates/holochain_sqlite/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p-0.1.0-beta-rc.0](crates/kitsune_p2p/CHANGELOG.md#0.1.0-beta-rc.0)

- **BREAKING CHANGE:** The gossip and publishing algorithms have undergone a significant rework, making this version incompatible with previous versions. Rather than gossiping and publishing entire Ops, only hashes are sent, which the recipient uses to maintain a queue of items which need to be fetched from various other sources on the DHT. This allows for finer-grained control over receiving Ops from multiple sources, and allows each node to manage their own incoming data flow. [\#1662](https://github.com/holochain/holochain/pull/1662)
- **BREAKING CHANGE:** `AppRequest::GossipInfo` is renamed to `AppRequest::NetworkInfo`, and the fields have changed. Since ops are no longer sent during gossip, there is no way to track overall gossip progress over a discrete time interval. There is now only a description of the total number of ops and total number of bytes waiting to be received. As ops are received, these numbers decrement.

## [kitsune\_p2p\_proxy-0.1.0-beta-rc.0](crates/kitsune_p2p_proxy/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p\_transport\_quic-0.1.0-beta-rc.0](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p\_mdns-0.1.0-beta-rc.0](crates/kitsune_p2p_mdns/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p\_fetch-0.0.1](crates/kitsune_p2p_fetch/CHANGELOG.md#0.0.1)

## [kitsune\_p2p\_types-0.1.0-beta-rc.0](crates/kitsune_p2p_types/CHANGELOG.md#0.1.0-beta-rc.0)

## [mr\_bundle-0.1.0-beta-rc.0](crates/mr_bundle/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_util-0.1.0-beta-rc.0](crates/holochain_util/CHANGELOG.md#0.1.0-beta-rc.0)

## [hdk-0.1.0-beta-rc.0](crates/hdk/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_zome\_types-0.1.0-beta-rc.0](crates/holochain_zome_types/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p\_dht-0.1.0-beta-rc.0](crates/kitsune_p2p_dht/CHANGELOG.md#0.1.0-beta-rc.0)

## [hdi-0.2.0-beta-rc.0](crates/hdi/CHANGELOG.md#0.2.0-beta-rc.0)

## [hdk\_derive-0.1.0-beta-rc.0](crates/hdk_derive/CHANGELOG.md#0.1.0-beta-rc.0)

## [holochain\_integrity\_types-0.1.0-beta-rc.0](crates/holochain_integrity_types/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p\_timestamp-0.1.0-beta-rc.0](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.1.0-beta-rc.0)

## [holo\_hash-0.1.0-beta-rc.0](crates/holo_hash/CHANGELOG.md#0.1.0-beta-rc.0)

## [kitsune\_p2p\_dht\_arc-0.1.0-beta-rc.0](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.1.0-beta-rc.0)

## [fixt-0.1.0-beta-rc.0](crates/fixt/CHANGELOG.md#0.1.0-beta-rc.0)

# 20221207.011003

## [holochain\_cli-0.0.71](crates/holochain_cli/CHANGELOG.md#0.0.71)

- Added handling of `hc` extensions. This allows for existing executables in the system whose names match `hc-<COMMAND>` to be executed with `hc <COMMAND>`.

# 20221130.011217

## [holochain\_cli-0.0.70](crates/holochain_cli/CHANGELOG.md#0.0.70)

## [holochain\_cli\_sandbox-0.0.66](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.66)

## [holochain\_cli\_bundle-0.0.65](crates/holochain_cli_bundle/CHANGELOG.md#0.0.65)

## [holochain-0.0.175](crates/holochain/CHANGELOG.md#0.0.175)

- BREAKING CHANGE - `ZomeId` and `zome_id` renamed to `ZomeIndex` and `zome_index` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppEntryType.id` renamed to `AppEntryType.entry_index` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppEntryType` renamed to `AppEntryDef` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppEntryDefName` renamed to `AppEntryName` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppRoleId` renamed to `RoleName` [\#1667](https://github.com/holochain/holochain/pull/1667)

## [holochain\_test\_wasm\_common-0.0.64](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.64)

## [holochain\_conductor\_api-0.0.72](crates/holochain_conductor_api/CHANGELOG.md#0.0.72)

## [holochain\_wasm\_test\_utils-0.0.71](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.71)

## [holochain\_cascade-0.0.74](crates/holochain_cascade/CHANGELOG.md#0.0.74)

## [holochain\_state-0.0.72](crates/holochain_state/CHANGELOG.md#0.0.72)

## [holochain\_p2p-0.0.69](crates/holochain_p2p/CHANGELOG.md#0.0.69)

## [holochain\_types-0.0.69](crates/holochain_types/CHANGELOG.md#0.0.69)

## [holochain\_keystore-0.0.67](crates/holochain_keystore/CHANGELOG.md#0.0.67)

## [holochain\_sqlite-0.0.66](crates/holochain_sqlite/CHANGELOG.md#0.0.66)

## [mr\_bundle-0.0.20](crates/mr_bundle/CHANGELOG.md#0.0.20)

## [hdk-0.0.163](crates/hdk/CHANGELOG.md#0.0.163)

## [holochain\_zome\_types-0.0.58](crates/holochain_zome_types/CHANGELOG.md#0.0.58)

## [hdi-0.1.10](crates/hdi/CHANGELOG.md#0.1.10)

## [hdk\_derive-0.0.56](crates/hdk_derive/CHANGELOG.md#0.0.56)

## [holochain\_integrity\_types-0.0.25](crates/holochain_integrity_types/CHANGELOG.md#0.0.25)

# 20221123.011302

## [holochain\_cli-0.0.69](crates/holochain_cli/CHANGELOG.md#0.0.69)

## [holochain\_cli\_sandbox-0.0.65](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.65)

## [holochain\_cli\_bundle-0.0.64](crates/holochain_cli_bundle/CHANGELOG.md#0.0.64)

## [holochain-0.0.174](crates/holochain/CHANGELOG.md#0.0.174)

- BREAKING CHANGE - The max entry size has been lowered to 4MB (strictly 4,000,000 bytes) [\#1659](https://github.com/holochain/holochain/pull/1659)
- BREAKING CHANGE - `emit_signal` permissions are changed so that it can be called during `post_commit`, which previously was not allowed [\#1661](https://github.com/holochain/holochain/pull/1661)

## [holochain\_test\_wasm\_common-0.0.63](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.63)

## [holochain\_conductor\_api-0.0.71](crates/holochain_conductor_api/CHANGELOG.md#0.0.71)

## [holochain\_wasm\_test\_utils-0.0.70](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.70)

## [holochain\_cascade-0.0.73](crates/holochain_cascade/CHANGELOG.md#0.0.73)

## [holochain\_state-0.0.71](crates/holochain_state/CHANGELOG.md#0.0.71)

## [holochain\_p2p-0.0.68](crates/holochain_p2p/CHANGELOG.md#0.0.68)

## [holochain\_types-0.0.68](crates/holochain_types/CHANGELOG.md#0.0.68)

## [holochain\_keystore-0.0.66](crates/holochain_keystore/CHANGELOG.md#0.0.66)

## [holochain\_sqlite-0.0.65](crates/holochain_sqlite/CHANGELOG.md#0.0.65)

## [kitsune\_p2p-0.0.52](crates/kitsune_p2p/CHANGELOG.md#0.0.52)

- The soft maximum gossip batch size has been lowered to 1MB (entries larger than this will just be in a batch alone), and the default timeouts have been increased from 30 seconds to 60 seconds. This is NOT a breaking change, though the usefulness is negated unless the majority of peers are running with the same settings.  [\#1659](https://github.com/holochain/holochain/pull/1659)

## [kitsune\_p2p\_proxy-0.0.39](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.39)

## [kitsune\_p2p\_transport\_quic-0.0.39](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.39)

## [kitsune\_p2p\_types-0.0.39](crates/kitsune_p2p_types/CHANGELOG.md#0.0.39)

## [mr\_bundle-0.0.19](crates/mr_bundle/CHANGELOG.md#0.0.19)

## [hdk-0.0.162](crates/hdk/CHANGELOG.md#0.0.162)

## [holochain\_zome\_types-0.0.57](crates/holochain_zome_types/CHANGELOG.md#0.0.57)

## [kitsune\_p2p\_dht-0.0.11](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.11)

## [hdi-0.1.9](crates/hdi/CHANGELOG.md#0.1.9)

## [hdk\_derive-0.0.55](crates/hdk_derive/CHANGELOG.md#0.0.55)

## [holochain\_integrity\_types-0.0.24](crates/holochain_integrity_types/CHANGELOG.md#0.0.24)

## [kitsune\_p2p\_timestamp-0.0.15](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.15)

# 20221116.012050

## [holochain\_cli-0.0.68](crates/holochain_cli/CHANGELOG.md#0.0.68)

## [holochain\_cli\_sandbox-0.0.64](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.64)

## [holochain\_cli\_bundle-0.0.63](crates/holochain_cli_bundle/CHANGELOG.md#0.0.63)

## [holochain-0.0.173](crates/holochain/CHANGELOG.md#0.0.173)

## [holochain\_test\_wasm\_common-0.0.62](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.62)

## [holochain\_conductor\_api-0.0.70](crates/holochain_conductor_api/CHANGELOG.md#0.0.70)

## [holochain\_wasm\_test\_utils-0.0.69](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.69)

## [holochain\_cascade-0.0.72](crates/holochain_cascade/CHANGELOG.md#0.0.72)

## [holochain\_state-0.0.70](crates/holochain_state/CHANGELOG.md#0.0.70)

## [holochain\_p2p-0.0.67](crates/holochain_p2p/CHANGELOG.md#0.0.67)

## [holochain\_types-0.0.67](crates/holochain_types/CHANGELOG.md#0.0.67)

## [holochain\_keystore-0.0.65](crates/holochain_keystore/CHANGELOG.md#0.0.65)

## [holochain\_sqlite-0.0.64](crates/holochain_sqlite/CHANGELOG.md#0.0.64)

## [kitsune\_p2p-0.0.51](crates/kitsune_p2p/CHANGELOG.md#0.0.51)

- `rpc_multi` now only actually makes a single request. This greatly simplifies the code path and eliminates a source of network bandwidth congestion, but removes the redundancy of aggregating the results of multiple peers. [\#1651](https://github.com/holochain/holochain/pull/1651)

## [kitsune\_p2p\_proxy-0.0.38](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.38)

## [kitsune\_p2p\_transport\_quic-0.0.38](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.38)

## [kitsune\_p2p\_types-0.0.38](crates/kitsune_p2p_types/CHANGELOG.md#0.0.38)

## [mr\_bundle-0.0.18](crates/mr_bundle/CHANGELOG.md#0.0.18)

## [holochain\_util-0.0.13](crates/holochain_util/CHANGELOG.md#0.0.13)

## [hdk-0.0.161](crates/hdk/CHANGELOG.md#0.0.161)

## [holochain\_zome\_types-0.0.56](crates/holochain_zome_types/CHANGELOG.md#0.0.56)

## [kitsune\_p2p\_dht-0.0.10](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.10)

# 20221109.012313

## [holochain\_cli-0.0.67](crates/holochain_cli/CHANGELOG.md#0.0.67)

## [holochain\_cli\_sandbox-0.0.63](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.63)

## [holochain\_cli\_bundle-0.0.62](crates/holochain_cli_bundle/CHANGELOG.md#0.0.62)

## [holochain-0.0.172](crates/holochain/CHANGELOG.md#0.0.172)

- BREAKING CHANGE - Update wasmer crate dependency [\#1620](https://github.com/holochain/holochain/pull/1620)

## [holochain\_test\_wasm\_common-0.0.61](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.61)

## [holochain\_conductor\_api-0.0.69](crates/holochain_conductor_api/CHANGELOG.md#0.0.69)

## [holochain\_wasm\_test\_utils-0.0.68](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.68)

## [holochain\_cascade-0.0.71](crates/holochain_cascade/CHANGELOG.md#0.0.71)

## [holochain\_state-0.0.69](crates/holochain_state/CHANGELOG.md#0.0.69)

## [holochain\_p2p-0.0.66](crates/holochain_p2p/CHANGELOG.md#0.0.66)

## [holochain\_types-0.0.66](crates/holochain_types/CHANGELOG.md#0.0.66)

## [holochain\_keystore-0.0.64](crates/holochain_keystore/CHANGELOG.md#0.0.64)

## [holochain\_sqlite-0.0.63](crates/holochain_sqlite/CHANGELOG.md#0.0.63)

## [hdk-0.0.160](crates/hdk/CHANGELOG.md#0.0.160)

## [holochain\_zome\_types-0.0.55](crates/holochain_zome_types/CHANGELOG.md#0.0.55)

**BREAKING CHANGE**: Rename `AuthorizeZomeCallSigningKey` to `GrantZomeCallCapability` & remove parameter `provenance`. [\#1647](https://github.com/holochain/holochain/pull/1647)

## [hdi-0.1.8](crates/hdi/CHANGELOG.md#0.1.8)

## [hdk\_derive-0.0.54](crates/hdk_derive/CHANGELOG.md#0.0.54)

# 20221103.145333

## [holochain\_cli-0.0.66](crates/holochain_cli/CHANGELOG.md#0.0.66)

## [holochain\_cli\_sandbox-0.0.62](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.62)

## [holochain\_cli\_bundle-0.0.61](crates/holochain_cli_bundle/CHANGELOG.md#0.0.61)

## [holochain-0.0.171](crates/holochain/CHANGELOG.md#0.0.171)

## [holochain\_test\_wasm\_common-0.0.60](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.60)

## [holochain\_conductor\_api-0.0.68](crates/holochain_conductor_api/CHANGELOG.md#0.0.68)

## [holochain\_wasm\_test\_utils-0.0.67](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.67)

## [holochain\_cascade-0.0.70](crates/holochain_cascade/CHANGELOG.md#0.0.70)

## [holochain\_state-0.0.68](crates/holochain_state/CHANGELOG.md#0.0.68)

## [holochain\_p2p-0.0.65](crates/holochain_p2p/CHANGELOG.md#0.0.65)

## [holochain\_types-0.0.65](crates/holochain_types/CHANGELOG.md#0.0.65)

- Fixed a bug where DNA modifiers specified in a hApp manifest would not be respected when specifying a `network_seed` in a `InstallAppBundlePayload`. [\#1642](https://github.com/holochain/holochain/pull/1642)

## [holochain\_keystore-0.0.63](crates/holochain_keystore/CHANGELOG.md#0.0.63)

## [holochain\_sqlite-0.0.62](crates/holochain_sqlite/CHANGELOG.md#0.0.62)

## [hdk-0.0.159](crates/hdk/CHANGELOG.md#0.0.159)

## [holochain\_zome\_types-0.0.54](crates/holochain_zome_types/CHANGELOG.md#0.0.54)

# 20221102.014648

## [holochain\_cli-0.0.65](crates/holochain_cli/CHANGELOG.md#0.0.65)

## [holochain\_cli\_sandbox-0.0.61](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.61)

## [holochain\_cli\_bundle-0.0.60](crates/holochain_cli_bundle/CHANGELOG.md#0.0.60)

## [holochain-0.0.170](crates/holochain/CHANGELOG.md#0.0.170)

- Add call to authorize a zome call signing key to Admin API [\#1641](https://github.com/holochain/holochain/pull/1641)
- Add call to request DNA definition to Admin API [\#1641](https://github.com/holochain/holochain/pull/1641)

## [holochain\_test\_wasm\_common-0.0.59](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.59)

## [holochain\_conductor\_api-0.0.67](crates/holochain_conductor_api/CHANGELOG.md#0.0.67)

## [holochain\_wasm\_test\_utils-0.0.66](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.66)

## [holochain\_cascade-0.0.69](crates/holochain_cascade/CHANGELOG.md#0.0.69)

## [holochain\_state-0.0.67](crates/holochain_state/CHANGELOG.md#0.0.67)

## [holochain\_p2p-0.0.64](crates/holochain_p2p/CHANGELOG.md#0.0.64)

## [holochain\_types-0.0.64](crates/holochain_types/CHANGELOG.md#0.0.64)

## [holochain\_keystore-0.0.62](crates/holochain_keystore/CHANGELOG.md#0.0.62)

## [holochain\_sqlite-0.0.61](crates/holochain_sqlite/CHANGELOG.md#0.0.61)

## [hdk-0.0.158](crates/hdk/CHANGELOG.md#0.0.158)

## [holochain\_zome\_types-0.0.53](crates/holochain_zome_types/CHANGELOG.md#0.0.53)

## [hdi-0.1.7](crates/hdi/CHANGELOG.md#0.1.7)

## [hdk\_derive-0.0.53](crates/hdk_derive/CHANGELOG.md#0.0.53)

## [holochain\_integrity\_types-0.0.23](crates/holochain_integrity_types/CHANGELOG.md#0.0.23)

## [holo\_hash-0.0.35](crates/holo_hash/CHANGELOG.md#0.0.35)

# 20221026.192152

## [holochain\_cli-0.0.64](crates/holochain_cli/CHANGELOG.md#0.0.64)

## [holochain\_cli\_sandbox-0.0.60](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.60)

## [holochain\_cli\_bundle-0.0.59](crates/holochain_cli_bundle/CHANGELOG.md#0.0.59)

- Adds `--recursive` command to `hc web-app pack` and `hc app pack` which packs all bundled dependencies for the given manifest. So `hc app pack ./workdir --recursive` will first go to each of the DNA manifests which have their location specified as bundled in the app manifest, pack each of them, and finally pack the app itself. `hc web-app pack ./workdir --recursive` will first pack the app recursively first if specified as bundled, and then pack the web-app manifest itself.

## [holochain-0.0.169](crates/holochain/CHANGELOG.md#0.0.169)

## [holochain\_test\_wasm\_common-0.0.58](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.58)

## [holochain\_conductor\_api-0.0.66](crates/holochain_conductor_api/CHANGELOG.md#0.0.66)

## [holochain\_wasm\_test\_utils-0.0.65](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.65)

## [holochain\_cascade-0.0.68](crates/holochain_cascade/CHANGELOG.md#0.0.68)

## [holochain\_state-0.0.66](crates/holochain_state/CHANGELOG.md#0.0.66)

## [holochain\_p2p-0.0.63](crates/holochain_p2p/CHANGELOG.md#0.0.63)

## [holochain\_types-0.0.63](crates/holochain_types/CHANGELOG.md#0.0.63)

## [holochain\_keystore-0.0.61](crates/holochain_keystore/CHANGELOG.md#0.0.61)

## [holochain\_sqlite-0.0.60](crates/holochain_sqlite/CHANGELOG.md#0.0.60)

## [kitsune\_p2p-0.0.50](crates/kitsune_p2p/CHANGELOG.md#0.0.50)

## [kitsune\_p2p\_proxy-0.0.37](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.37)

## [kitsune\_p2p\_transport\_quic-0.0.37](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.37)

## [kitsune\_p2p\_types-0.0.37](crates/kitsune_p2p_types/CHANGELOG.md#0.0.37)

## [hdk-0.0.157](crates/hdk/CHANGELOG.md#0.0.157)

- Pin the *hdi* dependency version. [\#1605](https://github.com/holochain/holochain/pull/1605)

## [holochain\_zome\_types-0.0.52](crates/holochain_zome_types/CHANGELOG.md#0.0.52)

## [kitsune\_p2p\_dht-0.0.9](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.9)

## [hdi-0.1.6](crates/hdi/CHANGELOG.md#0.1.6)

## [hdk\_derive-0.0.52](crates/hdk_derive/CHANGELOG.md#0.0.52)

## [holochain\_integrity\_types-0.0.22](crates/holochain_integrity_types/CHANGELOG.md#0.0.22)

# 20221019.014538

## [holochain\_cli-0.0.63](crates/holochain_cli/CHANGELOG.md#0.0.63)

## [holochain\_cli\_sandbox-0.0.59](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.59)

## [holochain\_cli\_bundle-0.0.58](crates/holochain_cli_bundle/CHANGELOG.md#0.0.58)

- Adds experimental `--raw` command to hc unpack commands (e.g. `hc dna unpack`) which allows an invalid manifest to still be unpacked. This can help to “salvage” a bundle which is no longer compatible with the current Holochain version, correcting the manifest so that it can be re-packed into a valid bundle.

## [holochain-0.0.168](crates/holochain/CHANGELOG.md#0.0.168)

- Fixes bug that causes crash when starting a conductor with a clone cell installed

## [holochain\_test\_wasm\_common-0.0.57](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.57)

## [holochain\_conductor\_api-0.0.65](crates/holochain_conductor_api/CHANGELOG.md#0.0.65)

## [holochain\_wasm\_test\_utils-0.0.64](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.64)

## [holochain\_cascade-0.0.67](crates/holochain_cascade/CHANGELOG.md#0.0.67)

## [holochain\_state-0.0.65](crates/holochain_state/CHANGELOG.md#0.0.65)

## [holochain\_p2p-0.0.62](crates/holochain_p2p/CHANGELOG.md#0.0.62)

## [holochain\_types-0.0.62](crates/holochain_types/CHANGELOG.md#0.0.62)

## [mr\_bundle-0.0.17](crates/mr_bundle/CHANGELOG.md#0.0.17)

## [hdk-0.0.156](crates/hdk/CHANGELOG.md#0.0.156)

## [hdi-0.1.5](crates/hdi/CHANGELOG.md#0.1.5)

# 20221012.015828

## [holochain\_cli-0.0.62](crates/holochain_cli/CHANGELOG.md#0.0.62)

## [holochain\_cli\_sandbox-0.0.58](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.58)

## [holochain\_cli\_bundle-0.0.57](crates/holochain_cli_bundle/CHANGELOG.md#0.0.57)

## [holochain-0.0.167](crates/holochain/CHANGELOG.md#0.0.167)

- Adds `SweetConductorConfig`, which adds a few builder methods for constructing variations of the standard ConductorConfig

## [holochain\_conductor\_api-0.0.64](crates/holochain_conductor_api/CHANGELOG.md#0.0.64)

## [holochain\_wasm\_test\_utils-0.0.63](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.63)

## [holochain\_cascade-0.0.66](crates/holochain_cascade/CHANGELOG.md#0.0.66)

## [holochain\_state-0.0.64](crates/holochain_state/CHANGELOG.md#0.0.64)

## [holochain\_p2p-0.0.61](crates/holochain_p2p/CHANGELOG.md#0.0.61)

## [holochain\_types-0.0.61](crates/holochain_types/CHANGELOG.md#0.0.61)

- Added `WebAppManifestCurrentBuilder` and exposed it.

## [holochain\_keystore-0.0.60](crates/holochain_keystore/CHANGELOG.md#0.0.60)

## [holochain\_sqlite-0.0.59](crates/holochain_sqlite/CHANGELOG.md#0.0.59)

## [kitsune\_p2p-0.0.49](crates/kitsune_p2p/CHANGELOG.md#0.0.49)

# 20221005.164304

## [holochain\_cli-0.0.61](crates/holochain_cli/CHANGELOG.md#0.0.61)

## [holochain\_cli\_sandbox-0.0.57](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.57)

## [holochain\_cli\_bundle-0.0.56](crates/holochain_cli_bundle/CHANGELOG.md#0.0.56)

## [holochain-0.0.166](crates/holochain/CHANGELOG.md#0.0.166)

- Fix restore clone cell by cell id. This used to fail with a “CloneCellNotFound” error. [\#1603](https://github.com/holochain/holochain/pull/1603)

## [holochain\_test\_wasm\_common-0.0.56](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.56)

## [holochain\_conductor\_api-0.0.63](crates/holochain_conductor_api/CHANGELOG.md#0.0.63)

## [holochain\_wasm\_test\_utils-0.0.62](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.62)

## [holochain\_cascade-0.0.65](crates/holochain_cascade/CHANGELOG.md#0.0.65)

## [holochain\_state-0.0.63](crates/holochain_state/CHANGELOG.md#0.0.63)

## [holochain\_p2p-0.0.60](crates/holochain_p2p/CHANGELOG.md#0.0.60)

## [holochain\_types-0.0.60](crates/holochain_types/CHANGELOG.md#0.0.60)

## [holochain\_keystore-0.0.59](crates/holochain_keystore/CHANGELOG.md#0.0.59)

## [holochain\_sqlite-0.0.58](crates/holochain_sqlite/CHANGELOG.md#0.0.58)

## [hdk-0.0.155](crates/hdk/CHANGELOG.md#0.0.155)

## [holochain\_zome\_types-0.0.51](crates/holochain_zome_types/CHANGELOG.md#0.0.51)

## [hdi-0.1.4](crates/hdi/CHANGELOG.md#0.1.4)

## [hdk\_derive-0.0.51](crates/hdk_derive/CHANGELOG.md#0.0.51)

## [holochain\_integrity\_types-0.0.21](crates/holochain_integrity_types/CHANGELOG.md#0.0.21)

## [holo\_hash-0.0.34](crates/holo_hash/CHANGELOG.md#0.0.34)

# 20220930.014733

## [holochain\_cli-0.0.60](crates/holochain_cli/CHANGELOG.md#0.0.60)

## [holochain\_cli\_sandbox-0.0.56](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.56)

## [holochain\_cli\_bundle-0.0.55](crates/holochain_cli_bundle/CHANGELOG.md#0.0.55)

## [holochain-0.0.165](crates/holochain/CHANGELOG.md#0.0.165)

- Revert requiring DNA modifiers when registering a DNA. These modifiers were optional before and were made mandatory by accident.

## [holochain\_conductor\_api-0.0.62](crates/holochain_conductor_api/CHANGELOG.md#0.0.62)

## [holochain\_wasm\_test\_utils-0.0.61](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.61)

## [holochain\_cascade-0.0.64](crates/holochain_cascade/CHANGELOG.md#0.0.64)

## [holochain\_state-0.0.62](crates/holochain_state/CHANGELOG.md#0.0.62)

## [holochain\_p2p-0.0.59](crates/holochain_p2p/CHANGELOG.md#0.0.59)

## [holochain\_types-0.0.59](crates/holochain_types/CHANGELOG.md#0.0.59)

## [holochain\_keystore-0.0.58](crates/holochain_keystore/CHANGELOG.md#0.0.58)

## [holochain\_sqlite-0.0.57](crates/holochain_sqlite/CHANGELOG.md#0.0.57)

## [kitsune\_p2p-0.0.48](crates/kitsune_p2p/CHANGELOG.md#0.0.48)

## [kitsune\_p2p\_proxy-0.0.36](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.36)

## [kitsune\_p2p\_transport\_quic-0.0.36](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.36)

## [kitsune\_p2p\_types-0.0.36](crates/kitsune_p2p_types/CHANGELOG.md#0.0.36)

# 20220928.014801

## [holochain\_cli-0.0.59](crates/holochain_cli/CHANGELOG.md#0.0.59)

## [holochain\_cli\_sandbox-0.0.55](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.55)

## [holochain\_cli\_bundle-0.0.54](crates/holochain_cli_bundle/CHANGELOG.md#0.0.54)

## [holochain-0.0.164](crates/holochain/CHANGELOG.md#0.0.164)

- Add App API call to archive an existing clone cell. [\#1578](https://github.com/holochain/holochain/pull/1578)
- Add Admin API call to restore an archived clone cell. [\#1578](https://github.com/holochain/holochain/pull/1578)
- Add Admin API call to delete all archived clone cells of an app’s role. For example, there is a base cell with role `document` and clones `document.0`, `document.1` etc.; this call deletes all clones permanently that have been archived before. This is not reversable; clones cannot be restored afterwards. [\#1578](https://github.com/holochain/holochain/pull/1578)

## [holochain\_test\_wasm\_common-0.0.55](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.55)

## [holochain\_conductor\_api-0.0.61](crates/holochain_conductor_api/CHANGELOG.md#0.0.61)

## [holochain\_wasm\_test\_utils-0.0.60](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.60)

## [holochain\_cascade-0.0.63](crates/holochain_cascade/CHANGELOG.md#0.0.63)

## [holochain\_state-0.0.61](crates/holochain_state/CHANGELOG.md#0.0.61)

## [holochain\_p2p-0.0.58](crates/holochain_p2p/CHANGELOG.md#0.0.58)

## [holochain\_types-0.0.58](crates/holochain_types/CHANGELOG.md#0.0.58)

- **BREAKING CHANGE**: `network_seed`, `origin_time` and `properties` are combined in a new struct `DnaModifiers`. API calls `RegisterDna`, `InstallAppBundle` and `CreateCloneCell` require this new struct as a substruct under the field `modifiers` now. [\#1578](https://github.com/holochain/holochain/pull/1578)
  - This means that all DNAs which set these fields will have to be rebuilt, and any code using the API will have to be updated (the @holochain/client Javascript client will be updated accordingly).
- **BREAKING CHANGE**: `origin_time` is a required field now in the `integrity` section of a DNA manifest.

## [holochain\_keystore-0.0.57](crates/holochain_keystore/CHANGELOG.md#0.0.57)

## [holochain\_sqlite-0.0.56](crates/holochain_sqlite/CHANGELOG.md#0.0.56)

## [kitsune\_p2p-0.0.47](crates/kitsune_p2p/CHANGELOG.md#0.0.47)

## [kitsune\_p2p\_proxy-0.0.35](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.35)

## [kitsune\_p2p\_transport\_quic-0.0.35](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.35)

## [kitsune\_p2p\_types-0.0.35](crates/kitsune_p2p_types/CHANGELOG.md#0.0.35)

## [hdk-0.0.154](crates/hdk/CHANGELOG.md#0.0.154)

## [holochain\_zome\_types-0.0.50](crates/holochain_zome_types/CHANGELOG.md#0.0.50)

- Revised the changelog for 0.0.48 to note that changes to `ChainQueryFilter` in that version were breaking changes, please read the log for that version for more detail.

## [kitsune\_p2p\_dht-0.0.8](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.8)

## [hdi-0.1.3](crates/hdi/CHANGELOG.md#0.1.3)

## [hdk\_derive-0.0.50](crates/hdk_derive/CHANGELOG.md#0.0.50)

## [holochain\_integrity\_types-0.0.20](crates/holochain_integrity_types/CHANGELOG.md#0.0.20)

## [holo\_hash-0.0.33](crates/holo_hash/CHANGELOG.md#0.0.33)

## [kitsune\_p2p\_dht\_arc-0.0.16](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.16)

# 20220921.145054

## [holochain\_cli-0.0.58](crates/holochain_cli/CHANGELOG.md#0.0.58)

## [holochain\_cli\_sandbox-0.0.54](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.54)

## [holochain\_cli\_bundle-0.0.53](crates/holochain_cli_bundle/CHANGELOG.md#0.0.53)

## [holochain-0.0.163](crates/holochain/CHANGELOG.md#0.0.163)

- Fixed rare “arc is not quantizable” panic, issuing a warning instead. [\#1577](https://github.com/holochain/holochain/pull/1577)

## [holochain\_test\_wasm\_common-0.0.54](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.54)

## [holochain\_conductor\_api-0.0.60](crates/holochain_conductor_api/CHANGELOG.md#0.0.60)

## [holochain\_wasm\_test\_utils-0.0.59](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.59)

## [holochain\_cascade-0.0.62](crates/holochain_cascade/CHANGELOG.md#0.0.62)

## [holochain\_state-0.0.60](crates/holochain_state/CHANGELOG.md#0.0.60)

## [holochain\_p2p-0.0.57](crates/holochain_p2p/CHANGELOG.md#0.0.57)

## [holochain\_types-0.0.57](crates/holochain_types/CHANGELOG.md#0.0.57)

- Renamed `SweetEasyInline` to `SweetInlineZomes`
- Renamed `InlineZome::callback` to `InlineZome::function`

## [holochain\_keystore-0.0.56](crates/holochain_keystore/CHANGELOG.md#0.0.56)

## [holochain\_sqlite-0.0.55](crates/holochain_sqlite/CHANGELOG.md#0.0.55)

## [kitsune\_p2p-0.0.46](crates/kitsune_p2p/CHANGELOG.md#0.0.46)

## [kitsune\_p2p\_proxy-0.0.34](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.34)

## [kitsune\_p2p\_transport\_quic-0.0.34](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.34)

## [kitsune\_p2p\_types-0.0.34](crates/kitsune_p2p_types/CHANGELOG.md#0.0.34)

## [hdk-0.0.153](crates/hdk/CHANGELOG.md#0.0.153)

## [holochain\_zome\_types-0.0.49](crates/holochain_zome_types/CHANGELOG.md#0.0.49)

## [kitsune\_p2p\_dht-0.0.7](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.7)

# 20220914.013149

## [holochain\_cli-0.0.57](crates/holochain_cli/CHANGELOG.md#0.0.57)

## [holochain\_cli\_sandbox-0.0.53](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.53)

## [holochain\_cli\_bundle-0.0.52](crates/holochain_cli_bundle/CHANGELOG.md#0.0.52)

## [holochain-0.0.162](crates/holochain/CHANGELOG.md#0.0.162)

- **BREAKING CHANGE**: Implement App API call `CreateCloneCell`. **Role ids must not contain a dot `.` any more.** Clone ids make use of the dot as a delimiter to separate role id and clone index. [\#1547](https://github.com/holochain/holochain/pull/1547)
- Remove conductor config legacy keystore config options. These config options have been broken since we removed legacy lair in \#1518, hence this fix itself is not a breaking change. Also adds the `lair_server_in_proc` keystore config option as the new default to run an embedded lair server inside the conductor process, no longer requiring a separate system process. [\#1571](https://github.com/holochain/holochain/pull/1571)

## [holochain\_test\_wasm\_common-0.0.53](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.53)

## [holochain\_conductor\_api-0.0.59](crates/holochain_conductor_api/CHANGELOG.md#0.0.59)

- Include cloned cells in App API call `AppInfo`. [\#1547](https://github.com/holochain/holochain/pull/1547)
- **BREAKING CHANGE:** The `AddRecords` admin api method has been changed to `GraftRecords`, and the functionality has changed accordingly. See the docs for that method to understand the changes.
  - In short, the `truncate` parameter has been removed. If you desire that functionality, simply pass a fully valid chain in for “grafting”, which will have the effect of removing all existing records. If you just want to append records to the existing chain, just pass in a collection of new records, with the first one pointing to the last existing record.

## [holochain\_wasm\_test\_utils-0.0.58](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.58)

## [holochain\_cascade-0.0.61](crates/holochain_cascade/CHANGELOG.md#0.0.61)

## [holochain\_state-0.0.59](crates/holochain_state/CHANGELOG.md#0.0.59)

## [holochain\_p2p-0.0.56](crates/holochain_p2p/CHANGELOG.md#0.0.56)

## [holochain\_types-0.0.56](crates/holochain_types/CHANGELOG.md#0.0.56)

- Add function to add a clone cell to an app. [\#1547](https://github.com/holochain/holochain/pull/1547)

## [holochain\_keystore-0.0.55](crates/holochain_keystore/CHANGELOG.md#0.0.55)

## [holochain\_sqlite-0.0.54](crates/holochain_sqlite/CHANGELOG.md#0.0.54)

## [kitsune\_p2p-0.0.45](crates/kitsune_p2p/CHANGELOG.md#0.0.45)

## [kitsune\_p2p\_proxy-0.0.33](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.33)

## [kitsune\_p2p\_transport\_quic-0.0.33](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.33)

## [kitsune\_p2p\_types-0.0.33](crates/kitsune_p2p_types/CHANGELOG.md#0.0.33)

## [mr\_bundle-0.0.16](crates/mr_bundle/CHANGELOG.md#0.0.16)

## [holochain\_util-0.0.12](crates/holochain_util/CHANGELOG.md#0.0.12)

## [hdk-0.0.152](crates/hdk/CHANGELOG.md#0.0.152)

## [holochain\_zome\_types-0.0.48](crates/holochain_zome_types/CHANGELOG.md#0.0.48)

- Add function to set DNA name. [\#1547](https://github.com/holochain/holochain/pull/1547)
- **BREAKING CHANGE** - `ChainQueryFilter` gets a new field, which may cause DNAs built with prior versions to break due to a deserialization error. Rebuild your DNA if so.
- There is now a `ChainQueryFilter::descending()` function which will cause the query results to be returned in descending order. This can be reversed by calling `ChainQueryFilter::ascending()`. The default order is still ascending. [\#1539](https://github.com/holochain/holochain/pull/1539)

## [kitsune\_p2p\_dht-0.0.6](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.6)

## [hdi-0.1.2](crates/hdi/CHANGELOG.md#0.1.2)

## [hdk\_derive-0.0.49](crates/hdk_derive/CHANGELOG.md#0.0.49)

## [holochain\_integrity\_types-0.0.19](crates/holochain_integrity_types/CHANGELOG.md#0.0.19)

## [kitsune\_p2p\_timestamp-0.0.14](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.14)

## [holo\_hash-0.0.32](crates/holo_hash/CHANGELOG.md#0.0.32)

## [kitsune\_p2p\_dht\_arc-0.0.15](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.15)

# 20220908.155008

## [holochain\_cli-0.0.56](crates/holochain_cli/CHANGELOG.md#0.0.56)

## [holochain\_cli\_sandbox-0.0.52](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.52)

## [holochain\_cli\_bundle-0.0.51](crates/holochain_cli_bundle/CHANGELOG.md#0.0.51)

## [holochain-0.0.161](crates/holochain/CHANGELOG.md#0.0.161)

## [holochain\_test\_wasm\_common-0.0.52](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.52)

## [holochain\_conductor\_api-0.0.58](crates/holochain_conductor_api/CHANGELOG.md#0.0.58)

## [holochain\_wasm\_test\_utils-0.0.57](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.57)

## [holochain\_cascade-0.0.60](crates/holochain_cascade/CHANGELOG.md#0.0.60)

## [holochain\_state-0.0.58](crates/holochain_state/CHANGELOG.md#0.0.58)

## [holochain\_p2p-0.0.55](crates/holochain_p2p/CHANGELOG.md#0.0.55)

## [holochain\_types-0.0.55](crates/holochain_types/CHANGELOG.md#0.0.55)

## [holochain\_keystore-0.0.54](crates/holochain_keystore/CHANGELOG.md#0.0.54)

## [holochain\_sqlite-0.0.53](crates/holochain_sqlite/CHANGELOG.md#0.0.53)

## [kitsune\_p2p-0.0.44](crates/kitsune_p2p/CHANGELOG.md#0.0.44)

- Fixes a regression where a node can prematurely end a gossip round if their partner signals that they are done sending data, even if the node itself still has more data to send, which can lead to persistent timeouts between the two nodes. [\#1553](https://github.com/holochain/holochain/pull/1553)

## [kitsune\_p2p\_proxy-0.0.32](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.32)

## [kitsune\_p2p\_transport\_quic-0.0.32](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.32)

## [kitsune\_p2p\_types-0.0.32](crates/kitsune_p2p_types/CHANGELOG.md#0.0.32)

## [hdk-0.0.151](crates/hdk/CHANGELOG.md#0.0.151)

## [holochain\_zome\_types-0.0.47](crates/holochain_zome_types/CHANGELOG.md#0.0.47)

## [kitsune\_p2p\_dht-0.0.5](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.5)

## [hdi-0.1.1](crates/hdi/CHANGELOG.md#0.1.1)

## [hdk\_derive-0.0.48](crates/hdk_derive/CHANGELOG.md#0.0.48)

## [holochain\_integrity\_types-0.0.18](crates/holochain_integrity_types/CHANGELOG.md#0.0.18)

# 20220907.100911

## [holochain-0.0.160](crates/holochain/CHANGELOG.md#0.0.160)

## [holochain\_test\_wasm\_common-0.0.51](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.51)

## [holochain\_cascade-0.0.59](crates/holochain_cascade/CHANGELOG.md#0.0.59)

## [hdk-0.0.150](crates/hdk/CHANGELOG.md#0.0.150)

## [hdi-0.1.0](crates/hdi/CHANGELOG.md#0.1.0)

- Initial minor version bump. This indicates our impression that we have made significant progress towards stabilizing the detereministic integrity layer’s API. [\#1550](https://github.com/holochain/holochain/pull/1550)

# 20220907.014838

## [holochain\_cli-0.0.55](crates/holochain_cli/CHANGELOG.md#0.0.55)

## [holochain\_cli\_sandbox-0.0.51](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.51)

## [holochain\_cli\_bundle-0.0.50](crates/holochain_cli_bundle/CHANGELOG.md#0.0.50)

## [holochain-0.0.159](crates/holochain/CHANGELOG.md#0.0.159)

- Updates TLS certificate handling so that multiple conductors can share the same lair, but use different TLS certificates by storing a “tag” in the conductor state database. This should not be a breaking change, but *will* result in a new TLS certificate being used per conductor. [\#1519](https://github.com/holochain/holochain/pull/1519)

## [holochain\_test\_wasm\_common-0.0.50](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.50)

## [holochain\_conductor\_api-0.0.57](crates/holochain_conductor_api/CHANGELOG.md#0.0.57)

## [holochain\_wasm\_test\_utils-0.0.56](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.56)

## [holochain\_cascade-0.0.58](crates/holochain_cascade/CHANGELOG.md#0.0.58)

## [holochain\_state-0.0.57](crates/holochain_state/CHANGELOG.md#0.0.57)

## [holochain\_p2p-0.0.54](crates/holochain_p2p/CHANGELOG.md#0.0.54)

## [holochain\_types-0.0.54](crates/holochain_types/CHANGELOG.md#0.0.54)

## [holochain\_keystore-0.0.53](crates/holochain_keystore/CHANGELOG.md#0.0.53)

- Add lair disconnect detection / reconnect loop with backoff for keystore resiliency. [\#1529](https://github.com/holochain/holochain/pull/1529)

## [holochain\_sqlite-0.0.52](crates/holochain_sqlite/CHANGELOG.md#0.0.52)

## [kitsune\_p2p-0.0.43](crates/kitsune_p2p/CHANGELOG.md#0.0.43)

- Increases all gossip bandwidth rate limits to 10mbps, up from 0.1mbps, allowing for gossip of larger entries
- Adds `gossip_burst_ratio` to `KitsuneTuningParams`, allowing tuning of bandwidth bursts
- Fixes a bug where a too-large gossip payload could put the rate limiter into an infinite loop

## [kitsune\_p2p\_proxy-0.0.31](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.31)

## [kitsune\_p2p\_transport\_quic-0.0.31](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.31)

## [kitsune\_p2p\_types-0.0.31](crates/kitsune_p2p_types/CHANGELOG.md#0.0.31)

## [hdk-0.0.149](crates/hdk/CHANGELOG.md#0.0.149)

## [holochain\_zome\_types-0.0.46](crates/holochain_zome_types/CHANGELOG.md#0.0.46)

## [kitsune\_p2p\_dht-0.0.4](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.4)

## [hdi-0.0.21](crates/hdi/CHANGELOG.md#0.0.21)

## [hdk\_derive-0.0.47](crates/hdk_derive/CHANGELOG.md#0.0.47)

## [holochain\_integrity\_types-0.0.17](crates/holochain_integrity_types/CHANGELOG.md#0.0.17)

## [kitsune\_p2p\_timestamp-0.0.13](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.13)

# 20220831.015922

## [holochain\_cli-0.0.54](crates/holochain_cli/CHANGELOG.md#0.0.54)

## [holochain\_cli\_sandbox-0.0.50](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.50)

## [holochain\_cli\_bundle-0.0.49](crates/holochain_cli_bundle/CHANGELOG.md#0.0.49)

## [holochain-0.0.158](crates/holochain/CHANGELOG.md#0.0.158)

## [holochain\_test\_wasm\_common-0.0.49](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.49)

## [holochain\_conductor\_api-0.0.56](crates/holochain_conductor_api/CHANGELOG.md#0.0.56)

## [holochain\_wasm\_test\_utils-0.0.55](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.55)

## [holochain\_cascade-0.0.57](crates/holochain_cascade/CHANGELOG.md#0.0.57)

## [holochain\_state-0.0.56](crates/holochain_state/CHANGELOG.md#0.0.56)

## [holochain\_p2p-0.0.53](crates/holochain_p2p/CHANGELOG.md#0.0.53)

## [holochain\_types-0.0.53](crates/holochain_types/CHANGELOG.md#0.0.53)

## [holochain\_keystore-0.0.52](crates/holochain_keystore/CHANGELOG.md#0.0.52)

## [holochain\_sqlite-0.0.51](crates/holochain_sqlite/CHANGELOG.md#0.0.51)

## [hdk-0.0.148](crates/hdk/CHANGELOG.md#0.0.148)

## [holochain\_zome\_types-0.0.45](crates/holochain_zome_types/CHANGELOG.md#0.0.45)

## [hdi-0.0.20](crates/hdi/CHANGELOG.md#0.0.20)

- Adds `must_get_agent_activity` which allows depending on an agents source chain by using a deterministic hash bounded range query. [\#1502](https://github.com/holochain/holochain/pull/1502)

## [hdk\_derive-0.0.46](crates/hdk_derive/CHANGELOG.md#0.0.46)

## [holochain\_integrity\_types-0.0.16](crates/holochain_integrity_types/CHANGELOG.md#0.0.16)

- Adds `ChainFilter` type for use in `must_get_agent_activity`. This allows specifying a chain top hash to start from and then creates a range either to genesis or `unit` a given hash or after `take`ing a number of actions. The range iterates backwards from the given chain top till it reaches on of the above possible chain bottoms. For this reason it will never contain forks. [\#1502](https://github.com/holochain/holochain/pull/1502)

# 20220824.014353

## [holochain\_cli-0.0.53](crates/holochain_cli/CHANGELOG.md#0.0.53)

## [holochain\_cli\_sandbox-0.0.49](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.49)

## [holochain-0.0.157](crates/holochain/CHANGELOG.md#0.0.157)

## [holochain\_conductor\_api-0.0.55](crates/holochain_conductor_api/CHANGELOG.md#0.0.55)

## [holochain\_cascade-0.0.56](crates/holochain_cascade/CHANGELOG.md#0.0.56)

## [holochain\_state-0.0.55](crates/holochain_state/CHANGELOG.md#0.0.55)

# 20220823.103320

## [holochain-0.0.156](crates/holochain/CHANGELOG.md#0.0.156)

- Effectively disable Wasm metering by setting the cranelift cost\_function to always return 0. This is meant as a temporary stop-gap and give us time to figure out a configurable approach. [\#1535](https://github.com/holochain/holochain/pull/1535)

# 20220820.111904

## [holochain\_cli-0.0.52](crates/holochain_cli/CHANGELOG.md#0.0.52)

## [holochain\_cli\_sandbox-0.0.48](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.48)

## [holochain\_cli\_bundle-0.0.48](crates/holochain_cli_bundle/CHANGELOG.md#0.0.48)

## [holochain-0.0.155](crates/holochain/CHANGELOG.md#0.0.155)

- **BREAKING CHANGE** - Removes legacy lair. You must now use lair-keystore \>= 0.2.0 with holochain. It is recommended to abandon your previous holochain agents, as there is not a straight forward migration path. To migrate: [dump the old keys](https://github.com/holochain/lair/blob/v0.0.11/crates/lair_keystore/src/bin/lair-keystore/main.rs#L38) -\> [write a utility to re-encode them](https://github.com/holochain/lair/tree/hc_seed_bundle-v0.1.2/crates/hc_seed_bundle) -\> [then import them to the new lair](https://github.com/holochain/lair/tree/lair_keystore-v0.2.0/crates/lair_keystore#lair-keystore-import-seed---help) – [\#1518](https://github.com/holochain/holochain/pull/1518)
- New solution for adding `hdi_version_req` field to the output of `--build-info` argument. [\#1523](https://github.com/holochain/holochain/pull/1523)

## [holochain\_test\_wasm\_common-0.0.48](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.48)

## [holochain\_conductor\_api-0.0.54](crates/holochain_conductor_api/CHANGELOG.md#0.0.54)

## [holochain\_wasm\_test\_utils-0.0.54](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.54)

## [holochain\_cascade-0.0.55](crates/holochain_cascade/CHANGELOG.md#0.0.55)

## [holochain\_state-0.0.54](crates/holochain_state/CHANGELOG.md#0.0.54)

## [holochain\_p2p-0.0.52](crates/holochain_p2p/CHANGELOG.md#0.0.52)

## [holochain\_types-0.0.52](crates/holochain_types/CHANGELOG.md#0.0.52)

## [holochain\_keystore-0.0.51](crates/holochain_keystore/CHANGELOG.md#0.0.51)

## [holochain\_sqlite-0.0.50](crates/holochain_sqlite/CHANGELOG.md#0.0.50)

## [kitsune\_p2p-0.0.42](crates/kitsune_p2p/CHANGELOG.md#0.0.42)

## [kitsune\_p2p\_proxy-0.0.30](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.30)

## [kitsune\_p2p\_transport\_quic-0.0.30](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.30)

## [kitsune\_p2p\_types-0.0.30](crates/kitsune_p2p_types/CHANGELOG.md#0.0.30)

## [hdk-0.0.147](crates/hdk/CHANGELOG.md#0.0.147)

## [hdi-0.0.19](crates/hdi/CHANGELOG.md#0.0.19)

## [hdk\_derive-0.0.45](crates/hdk_derive/CHANGELOG.md#0.0.45)

# 20220817.013233

## [holochain\_cli-0.0.51](crates/holochain_cli/CHANGELOG.md#0.0.51)

## [holochain\_cli\_sandbox-0.0.47](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.47)

- **BREAKING CHANGE** - `hc sandbox` updated to use new (0.y.z) lair api. Any old sandboxes will no longer function. It is recommended to create new sandboxes, as there is not a straight forward migration path. To migrate: [dump the old keys](https://github.com/holochain/lair/blob/v0.0.11/crates/lair_keystore/src/bin/lair-keystore/main.rs#L38) -\> [write a utility to re-encode them](https://github.com/holochain/lair/tree/hc_seed_bundle-v0.1.2/crates/hc_seed_bundle) -\> [then import them to the new lair](https://github.com/holochain/lair/tree/lair_keystore-v0.2.0/crates/lair_keystore#lair-keystore-import-seed---help) – [\#1515](https://github.com/holochain/holochain/pull/1515)

## [holochain\_cli\_bundle-0.0.47](crates/holochain_cli_bundle/CHANGELOG.md#0.0.47)

## [holochain-0.0.154](crates/holochain/CHANGELOG.md#0.0.154)

- Revert: “Add the `hdi_version_req` key:value field to the output of the `--build-info` argument” because it broke. [\#1521](https://github.com/holochain/holochain/pull/1521)
  
  Reason: it causes a build failure of the *holochain*  crate on crates.io

## [holochain\_test\_wasm\_common-0.0.47](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.47)

## [holochain\_conductor\_api-0.0.53](crates/holochain_conductor_api/CHANGELOG.md#0.0.53)

## [holochain\_wasm\_test\_utils-0.0.53](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.53)

## [holochain\_cascade-0.0.54](crates/holochain_cascade/CHANGELOG.md#0.0.54)

## [holochain\_state-0.0.53](crates/holochain_state/CHANGELOG.md#0.0.53)

## [holochain\_p2p-0.0.51](crates/holochain_p2p/CHANGELOG.md#0.0.51)

## [holochain\_types-0.0.51](crates/holochain_types/CHANGELOG.md#0.0.51)

## [holochain\_keystore-0.0.50](crates/holochain_keystore/CHANGELOG.md#0.0.50)

## [holochain\_sqlite-0.0.49](crates/holochain_sqlite/CHANGELOG.md#0.0.49)

## [kitsune\_p2p-0.0.41](crates/kitsune_p2p/CHANGELOG.md#0.0.41)

## [kitsune\_p2p\_proxy-0.0.29](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.29)

## [kitsune\_p2p\_transport\_quic-0.0.29](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.29)

## [kitsune\_p2p\_types-0.0.29](crates/kitsune_p2p_types/CHANGELOG.md#0.0.29)

## [mr\_bundle-0.0.15](crates/mr_bundle/CHANGELOG.md#0.0.15)

## [holochain\_util-0.0.11](crates/holochain_util/CHANGELOG.md#0.0.11)

## [hdk-0.0.146](crates/hdk/CHANGELOG.md#0.0.146)

## [holochain\_zome\_types-0.0.44](crates/holochain_zome_types/CHANGELOG.md#0.0.44)

## [kitsune\_p2p\_dht-0.0.3](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.3)

## [hdi-0.0.18](crates/hdi/CHANGELOG.md#0.0.18)

## [hdk\_derive-0.0.44](crates/hdk_derive/CHANGELOG.md#0.0.44)

## [holochain\_integrity\_types-0.0.15](crates/holochain_integrity_types/CHANGELOG.md#0.0.15)

## [kitsune\_p2p\_timestamp-0.0.12](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.12)

# 20220810.012252

## [holochain-0.0.153](crates/holochain/CHANGELOG.md#0.0.153)

- Add the `hdi_version_req` key:value field to the output of the `--build-info` argument

## [holochain\_test\_wasm\_common-0.0.46](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.46)

## [holochain\_wasm\_test\_utils-0.0.52](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.52)

## [holochain\_cascade-0.0.53](crates/holochain_cascade/CHANGELOG.md#0.0.53)

## [hdk-0.0.145](crates/hdk/CHANGELOG.md#0.0.145)

## [hdi-0.0.17](crates/hdi/CHANGELOG.md#0.0.17)

# 20220803.124141

## [holochain\_cli-0.0.50](crates/holochain_cli/CHANGELOG.md#0.0.50)

## [holochain\_cli\_sandbox-0.0.46](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.46)

## [holochain\_cli\_bundle-0.0.46](crates/holochain_cli_bundle/CHANGELOG.md#0.0.46)

## [holochain-0.0.152](crates/holochain/CHANGELOG.md#0.0.152)

- Adds `AdminRequest::UpdateCoordinators` that allows swapping coordinator zomes for a running happ.

## [holochain\_test\_wasm\_common-0.0.45](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.45)

## [holochain\_conductor\_api-0.0.52](crates/holochain_conductor_api/CHANGELOG.md#0.0.52)

## [holochain\_wasm\_test\_utils-0.0.51](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.51)

## [holochain\_cascade-0.0.52](crates/holochain_cascade/CHANGELOG.md#0.0.52)

## [holochain\_state-0.0.52](crates/holochain_state/CHANGELOG.md#0.0.52)

## [holochain\_p2p-0.0.50](crates/holochain_p2p/CHANGELOG.md#0.0.50)

## [holochain\_types-0.0.50](crates/holochain_types/CHANGELOG.md#0.0.50)

## [holochain\_keystore-0.0.49](crates/holochain_keystore/CHANGELOG.md#0.0.49)

## [holochain\_sqlite-0.0.48](crates/holochain_sqlite/CHANGELOG.md#0.0.48)

## [hdk-0.0.144](crates/hdk/CHANGELOG.md#0.0.144)

- Docs: Add example how to get a typed path from a path to `path` module [\#1505](https://github.com/holochain/holochain/pull/1505)
- Exposed `TypedPath` type in the hdk prelude for easy access from zomes.

## [holochain\_zome\_types-0.0.43](crates/holochain_zome_types/CHANGELOG.md#0.0.43)

## [hdi-0.0.16](crates/hdi/CHANGELOG.md#0.0.16)

- Docs: Add `OpType` helper example to HDI validation section [\#1505](https://github.com/holochain/holochain/pull/1505)

## [hdk\_derive-0.0.43](crates/hdk_derive/CHANGELOG.md#0.0.43)

## [holochain\_integrity\_types-0.0.14](crates/holochain_integrity_types/CHANGELOG.md#0.0.14)

# 20220728.122329

- nix-shell: exclude most holonix components by default to reduce shell size [\#1498](https://github.com/holochain/holochain/pull/1498)

## [holochain\_cli-0.0.49](crates/holochain_cli/CHANGELOG.md#0.0.49)

## [holochain\_cli\_sandbox-0.0.45](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.45)

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## [holochain\_cli\_bundle-0.0.45](crates/holochain_cli_bundle/CHANGELOG.md#0.0.45)

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## [holochain-0.0.151](crates/holochain/CHANGELOG.md#0.0.151)

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)
- Allow deterministic bindings (dna\_info() & zome\_info()) to the genesis self check [\#1491](https://github.com/holochain/holochain/pull/1491).

## [holochain\_test\_wasm\_common-0.0.44](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.44)

## [holochain\_conductor\_api-0.0.51](crates/holochain_conductor_api/CHANGELOG.md#0.0.51)

## [holochain\_wasm\_test\_utils-0.0.50](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.50)

## [holochain\_cascade-0.0.51](crates/holochain_cascade/CHANGELOG.md#0.0.51)

## [holochain\_state-0.0.51](crates/holochain_state/CHANGELOG.md#0.0.51)

## [holochain\_p2p-0.0.49](crates/holochain_p2p/CHANGELOG.md#0.0.49)

## [holochain\_types-0.0.49](crates/holochain_types/CHANGELOG.md#0.0.49)

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## [holochain\_keystore-0.0.48](crates/holochain_keystore/CHANGELOG.md#0.0.48)

## [holochain\_sqlite-0.0.47](crates/holochain_sqlite/CHANGELOG.md#0.0.47)

## [kitsune\_p2p-0.0.40](crates/kitsune_p2p/CHANGELOG.md#0.0.40)

## [kitsune\_p2p\_proxy-0.0.28](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.28)

## [kitsune\_p2p\_transport\_quic-0.0.28](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.28)

## [kitsune\_p2p\_types-0.0.28](crates/kitsune_p2p_types/CHANGELOG.md#0.0.28)

## [mr\_bundle-0.0.14](crates/mr_bundle/CHANGELOG.md#0.0.14)

- Fix inconsistent bundle writting due to unordered map of bundle resources

## [hdk-0.0.143](crates/hdk/CHANGELOG.md#0.0.143)

- Docs: Add documentation on `get_links` argument `link_type`. [\#1486](https://github.com/holochain/holochain/pull/1486)
- Docs: Intra-link to `wasm_error` and `WasmErrorInner`. [\#1486](https://github.com/holochain/holochain/pull/1486)

## [holochain\_zome\_types-0.0.42](crates/holochain_zome_types/CHANGELOG.md#0.0.42)

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## [kitsune\_p2p\_dht-0.0.2](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.2)

## [hdi-0.0.15](crates/hdi/CHANGELOG.md#0.0.15)

- Adds the `OpHelper` trait to create the `OpType` convenience type to help with writing validation code. [\#1488](https://github.com/holochain/holochain/pull/1488)
- Docs: Add documentation on `LinkTypeFilterExt`. [\#1486](https://github.com/holochain/holochain/pull/1486)

## [hdk\_derive-0.0.42](crates/hdk_derive/CHANGELOG.md#0.0.42)

## [holochain\_integrity\_types-0.0.13](crates/holochain_integrity_types/CHANGELOG.md#0.0.13)

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## [holo\_hash-0.0.31](crates/holo_hash/CHANGELOG.md#0.0.31)

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## [kitsune\_p2p\_dht\_arc-0.0.14](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.14)

## [fixt-0.0.14](crates/fixt/CHANGELOG.md#0.0.14)

# 20220713.013021

- **BREAKING**: the `holochain_deterministic_integrity` crate has been renamed to `hdi`

## [holochain\_cli-0.0.48](crates/holochain_cli/CHANGELOG.md#0.0.48)

## [holochain\_cli\_sandbox-0.0.44](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.44)

## [holochain\_cli\_bundle-0.0.44](crates/holochain_cli_bundle/CHANGELOG.md#0.0.44)

## [holochain-0.0.150](crates/holochain/CHANGELOG.md#0.0.150)

## [holochain\_test\_wasm\_common-0.0.43](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.43)

## [holochain\_conductor\_api-0.0.50](crates/holochain_conductor_api/CHANGELOG.md#0.0.50)

## [holochain\_wasm\_test\_utils-0.0.49](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.49)

## [holochain\_cascade-0.0.50](crates/holochain_cascade/CHANGELOG.md#0.0.50)

## [holochain\_state-0.0.50](crates/holochain_state/CHANGELOG.md#0.0.50)

## [holochain\_p2p-0.0.48](crates/holochain_p2p/CHANGELOG.md#0.0.48)

## [holochain\_types-0.0.48](crates/holochain_types/CHANGELOG.md#0.0.48)

## [holochain\_keystore-0.0.47](crates/holochain_keystore/CHANGELOG.md#0.0.47)

## [holochain\_sqlite-0.0.46](crates/holochain_sqlite/CHANGELOG.md#0.0.46)

## [hdk-0.0.142](crates/hdk/CHANGELOG.md#0.0.142)

## [holochain\_zome\_types-0.0.41](crates/holochain_zome_types/CHANGELOG.md#0.0.41)

## [hdi-0.0.14](crates/hdi/CHANGELOG.md#0.0.14)

- Docs: replace occurrences of `hdk_entry_def` and `entry_def!` with `hdk_entry_helper`.

## [hdk\_derive-0.0.41](crates/hdk_derive/CHANGELOG.md#0.0.41)

# 20220710.155915

## [holochain\_cli-0.0.47](crates/holochain_cli/CHANGELOG.md#0.0.47)

## [holochain\_cli\_sandbox-0.0.43](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.43)

## [holochain\_cli\_bundle-0.0.43](crates/holochain_cli_bundle/CHANGELOG.md#0.0.43)

## [holochain-0.0.149](crates/holochain/CHANGELOG.md#0.0.149)

## [holochain\_test\_wasm\_common-0.0.42](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.42)

## [holochain\_conductor\_api-0.0.49](crates/holochain_conductor_api/CHANGELOG.md#0.0.49)

## [holochain\_wasm\_test\_utils-0.0.48](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.48)

## [holochain\_cascade-0.0.49](crates/holochain_cascade/CHANGELOG.md#0.0.49)

## [holochain\_state-0.0.49](crates/holochain_state/CHANGELOG.md#0.0.49)

## [holochain\_p2p-0.0.47](crates/holochain_p2p/CHANGELOG.md#0.0.47)

## [holochain\_types-0.0.47](crates/holochain_types/CHANGELOG.md#0.0.47)

## [holochain\_keystore-0.0.46](crates/holochain_keystore/CHANGELOG.md#0.0.46)

## [holochain\_sqlite-0.0.45](crates/holochain_sqlite/CHANGELOG.md#0.0.45)

## [hdk-0.0.141](crates/hdk/CHANGELOG.md#0.0.141)

- Docs: Add section on coordinator zomes and link to HDI crate.

## [holochain\_zome\_types-0.0.40](crates/holochain_zome_types/CHANGELOG.md#0.0.40)

## [holochain\_deterministic\_integrity-0.0.13](crates/hdi/CHANGELOG.md#0.0.13)

- Docs: crate level documentation for `holochain_deterministic_integrity`.

### Added

## [hdk\_derive-0.0.40](crates/hdk_derive/CHANGELOG.md#0.0.40)

## [holochain\_integrity\_types-0.0.12](crates/holochain_integrity_types/CHANGELOG.md#0.0.12)

# 20220706.013407

## [holochain\_cli-0.0.46](crates/holochain_cli/CHANGELOG.md#0.0.46)

## [holochain\_cli\_sandbox-0.0.42](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.42)

## [holochain\_cli\_bundle-0.0.42](crates/holochain_cli_bundle/CHANGELOG.md#0.0.42)

## [holochain-0.0.148](crates/holochain/CHANGELOG.md#0.0.148)

- Added networking logic for enzymatic countersigning [\#1472](https://github.com/holochain/holochain/pull/1472)
- Countersigning authority response network message changed to a session negotiation enum [/\#1472](https://github.com/holochain/holochain/pull/1472)

## [holochain\_conductor\_api-0.0.48](crates/holochain_conductor_api/CHANGELOG.md#0.0.48)

## [holochain\_wasm\_test\_utils-0.0.47](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.47)

## [holochain\_cascade-0.0.48](crates/holochain_cascade/CHANGELOG.md#0.0.48)

## [holochain\_state-0.0.48](crates/holochain_state/CHANGELOG.md#0.0.48)

## [holochain\_p2p-0.0.46](crates/holochain_p2p/CHANGELOG.md#0.0.46)

## [holochain\_types-0.0.46](crates/holochain_types/CHANGELOG.md#0.0.46)

# 20220701.181019

## [holochain\_cli-0.0.45](crates/holochain_cli/CHANGELOG.md#0.0.45)

## [holochain\_cli\_sandbox-0.0.41](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.41)

## [holochain\_cli\_bundle-0.0.41](crates/holochain_cli_bundle/CHANGELOG.md#0.0.41)

## [holochain-0.0.147](crates/holochain/CHANGELOG.md#0.0.147)

## [holochain\_test\_wasm\_common-0.0.41](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.41)

## [holochain\_conductor\_api-0.0.47](crates/holochain_conductor_api/CHANGELOG.md#0.0.47)

## [holochain\_wasm\_test\_utils-0.0.46](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.46)

## [holochain\_cascade-0.0.47](crates/holochain_cascade/CHANGELOG.md#0.0.47)

## [holochain\_state-0.0.47](crates/holochain_state/CHANGELOG.md#0.0.47)

## [holochain\_p2p-0.0.45](crates/holochain_p2p/CHANGELOG.md#0.0.45)

## [holochain\_types-0.0.45](crates/holochain_types/CHANGELOG.md#0.0.45)

## [holochain\_keystore-0.0.45](crates/holochain_keystore/CHANGELOG.md#0.0.45)

## [holochain\_sqlite-0.0.44](crates/holochain_sqlite/CHANGELOG.md#0.0.44)

## [kitsune\_p2p-0.0.39](crates/kitsune_p2p/CHANGELOG.md#0.0.39)

## [kitsune\_p2p\_proxy-0.0.27](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.27)

## [kitsune\_p2p\_transport\_quic-0.0.27](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.27)

## [kitsune\_p2p\_types-0.0.27](crates/kitsune_p2p_types/CHANGELOG.md#0.0.27)

## [hdk-0.0.140](crates/hdk/CHANGELOG.md#0.0.140)

## [holochain\_zome\_types-0.0.39](crates/holochain_zome_types/CHANGELOG.md#0.0.39)

## [kitsune\_p2p\_dht-0.0.1](crates/kitsune_p2p_dht/CHANGELOG.md#0.0.1)

## [holochain\_deterministic\_integrity-0.0.12](crates/hdi/CHANGELOG.md#0.0.12)

## [hdk\_derive-0.0.39](crates/hdk_derive/CHANGELOG.md#0.0.39)

## [holochain\_integrity\_types-0.0.11](crates/holochain_integrity_types/CHANGELOG.md#0.0.11)

## [kitsune\_p2p\_timestamp-0.0.11](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.11)

## [holo\_hash-0.0.30](crates/holo_hash/CHANGELOG.md#0.0.30)

## [kitsune\_p2p\_dht\_arc-0.0.13](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.13)

# 20220629.012044

## [holochain\_cli-0.0.44](crates/holochain_cli/CHANGELOG.md#0.0.44)

## [holochain\_cli\_sandbox-0.0.40](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.40)

## [holochain\_cli\_bundle-0.0.40](crates/holochain_cli_bundle/CHANGELOG.md#0.0.40)

## [holochain-0.0.146](crates/holochain/CHANGELOG.md#0.0.146)

## [holochain\_test\_wasm\_common-0.0.40](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.40)

## [holochain\_conductor\_api-0.0.46](crates/holochain_conductor_api/CHANGELOG.md#0.0.46)

## [holochain\_wasm\_test\_utils-0.0.45](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.45)

## [holochain\_cascade-0.0.46](crates/holochain_cascade/CHANGELOG.md#0.0.46)

## [holochain\_state-0.0.46](crates/holochain_state/CHANGELOG.md#0.0.46)

## [holochain\_p2p-0.0.44](crates/holochain_p2p/CHANGELOG.md#0.0.44)

## [holochain\_types-0.0.44](crates/holochain_types/CHANGELOG.md#0.0.44)

## [holochain\_keystore-0.0.44](crates/holochain_keystore/CHANGELOG.md#0.0.44)

## [holochain\_sqlite-0.0.43](crates/holochain_sqlite/CHANGELOG.md#0.0.43)

## [kitsune\_p2p-0.0.38](crates/kitsune_p2p/CHANGELOG.md#0.0.38)

## [kitsune\_p2p\_proxy-0.0.26](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.26)

## [kitsune\_p2p\_transport\_quic-0.0.26](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.26)

## [kitsune\_p2p\_types-0.0.26](crates/kitsune_p2p_types/CHANGELOG.md#0.0.26)

## [hdk-0.0.139](crates/hdk/CHANGELOG.md#0.0.139)

- **BREAKING CHANGE:** Anchor functions, `TypedPath` and `create_link` take `ScopedLinkType: TryFrom<T>` instead of `LinkType: From<T>`.
- **BREAKING CHANGE:** `create_entry` takes `ScopedEntryDefIndex: TryFrom<T>` instead of `EntryDefIndex: TryFrom<T>`.
- **BREAKING CHANGE:** `get_links` and `get_link_details` take `impl LinkTypeFilterExt` instead of `TryInto<LinkTypeRanges>`.
- hdk: **BREAKING CHANGE** `x_salsa20_poly1305_*` functions have been properly implemented. Any previous `KeyRef`s will no longer work. These new functions DO NOT work with legacy lair `v0.0.z`, you must use NEW lair `v0.y.z` (v0.2.0 as of this PR). [\#1446](https://github.com/holochain/holochain/pull/1446)

## [holochain\_zome\_types-0.0.38](crates/holochain_zome_types/CHANGELOG.md#0.0.38)

## [holochain\_deterministic\_integrity-0.0.11](crates/hdi/CHANGELOG.md#0.0.11)

- `EntryTypesHelper`: `try_from_local_type` is removed and `try_from_global_type` becomes `deserialize_from_type`.
- `LinkTypesHelper` is removed.
- `LinkTypeFilterExt` is added to allow extra types to convert to `LinkTypeFilter`.

## [hdk\_derive-0.0.38](crates/hdk_derive/CHANGELOG.md#0.0.38)

- `hdk_to_global_types` is removed.
- `hdk_to_local_types` becomes `hdk_to_coordinates`.

## [holochain\_integrity\_types-0.0.10](crates/holochain_integrity_types/CHANGELOG.md#0.0.10)

- `ZomeId` added back to `CreateLink` and `AppEntryType`.
- `ScopedZomeTypesSet` has been simplified for easier use. Global and local types have been removed in favor of scoping `EntryDefIndex` and `LinkType` with the `ZomeId` of where they were defined.
- `LinkTypeRanges` has been removed.
- `LinkTypeFilter` replaces `LinkTypeRanges` as a more simplified way of filtering on `get_links`. `..` can be used to get all links from a zomes dependencies.
- `GlobalZomeTypeId` and `LocalZomeTypeId` removed.
- Links from integrity zomes that are not part of a coordinators dependency list are no longer accessible.
- In preparation for rate limiting, the inner Action structs which support app-defined “weights”, viz. `Create`, `Update`, `Delete`, and `CreateLink`, now have a `weight` field. This is currently set to a default value of “no weight”, but will later be used to store the app-defined weight.
  - A bit of deeper detail on this change: each of these action structs is now generic over the weight field, to allow “weighed” and “unweighed” versions of that header. This is necessary to be able to express these actions both before and after they have undergone the weighing process.

# 20220622.133046

## [holochain\_cli-0.0.43](crates/holochain_cli/CHANGELOG.md#0.0.43)

## [holochain\_cli\_sandbox-0.0.39](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.39)

## [holochain\_cli\_bundle-0.0.39](crates/holochain_cli_bundle/CHANGELOG.md#0.0.39)

## [holochain-0.0.145](crates/holochain/CHANGELOG.md#0.0.145)

**MAJOR BREAKING CHANGE\!** This release includes a rename of two Holochain core concepts, which results in a LOT of changes to public APIs and type names:

- “Element” has been renamed to “Record”
- “Header” has been renamed to “Action”

All names which include these words have also been renamed accordingly.

As Holochain has evolved, the meaning behind these concepts, as well as our understanding of them, has evolved as well, to the point that the original names are no longer adequate descriptors. We chose new names to help better reflect what these concepts mean, to bring more clarity to how we write and talk about Holochain.

## [holochain\_test\_wasm\_common-0.0.39](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.39)

## [holochain\_conductor\_api-0.0.45](crates/holochain_conductor_api/CHANGELOG.md#0.0.45)

## [holochain\_wasm\_test\_utils-0.0.44](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.44)

## [holochain\_cascade-0.0.45](crates/holochain_cascade/CHANGELOG.md#0.0.45)

## [holochain\_state-0.0.45](crates/holochain_state/CHANGELOG.md#0.0.45)

## [holochain\_p2p-0.0.43](crates/holochain_p2p/CHANGELOG.md#0.0.43)

## [holochain\_types-0.0.43](crates/holochain_types/CHANGELOG.md#0.0.43)

## [holochain\_keystore-0.0.43](crates/holochain_keystore/CHANGELOG.md#0.0.43)

## [holochain\_sqlite-0.0.42](crates/holochain_sqlite/CHANGELOG.md#0.0.42)

## [kitsune\_p2p-0.0.37](crates/kitsune_p2p/CHANGELOG.md#0.0.37)

## [kitsune\_p2p\_proxy-0.0.25](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.25)

## [kitsune\_p2p\_transport\_quic-0.0.25](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.25)

## [kitsune\_p2p\_types-0.0.25](crates/kitsune_p2p_types/CHANGELOG.md#0.0.25)

## [hdk-0.0.138](crates/hdk/CHANGELOG.md#0.0.138)

- hdk: Bump rand version + fix getrandom (used by rand\_core and rand) to fetch randomness from host system when compiled to WebAssembly. [\#1445](https://github.com/holochain/holochain/pull/1445)

## [holochain\_zome\_types-0.0.37](crates/holochain_zome_types/CHANGELOG.md#0.0.37)

## [holochain\_deterministic\_integrity-0.0.10](crates/hdi/CHANGELOG.md#0.0.10)

## [hdk\_derive-0.0.37](crates/hdk_derive/CHANGELOG.md#0.0.37)

## [holochain\_integrity\_types-0.0.9](crates/holochain_integrity_types/CHANGELOG.md#0.0.9)

- Countersigning now accepts optional additional signers but the first must be the enzyme [\#1394](https://github.com/holochain/holochain/pull/1394)
- The first agent in countersigning is always the enzyme if enzymatic [\#1394](https://github.com/holochain/holochain/pull/1394)

## [kitsune\_p2p\_timestamp-0.0.10](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.10)

## [holo\_hash-0.0.29](crates/holo_hash/CHANGELOG.md#0.0.29)

## [kitsune\_p2p\_dht\_arc-0.0.12](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.12)

## [fixt-0.0.13](crates/fixt/CHANGELOG.md#0.0.13)

# 20220616.084359

- Docs: Update OS support in repository README and link to developer environment setup.

## [holochain\_cli-0.0.42](crates/holochain_cli/CHANGELOG.md#0.0.42)

## [holochain\_cli\_sandbox-0.0.38](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.38)

## [holochain\_cli\_bundle-0.0.38](crates/holochain_cli_bundle/CHANGELOG.md#0.0.38)

## [holochain-0.0.144](crates/holochain/CHANGELOG.md#0.0.144)

- Add functional stub for `x_salsa20_poly1305_shared_secret_create_random` [\#1410](https://github.com/holochain/holochain/pull/1410)
- Add functional stub for `x_salsa20_poly1305_shared_secret_export` [\#1410](https://github.com/holochain/holochain/pull/1410)
- Add functional stub for `x_salsa20_poly1305_shared_secret_ingest` [\#1410](https://github.com/holochain/holochain/pull/1410)
- Limit conductor calls to `10_000_000_000` Wasm operations [\#1386](https://github.com/holochain/holochain/pull/1386)

## [holochain\_test\_wasm\_common-0.0.38](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.38)

## [holochain\_conductor\_api-0.0.44](crates/holochain_conductor_api/CHANGELOG.md#0.0.44)

## [holochain\_wasm\_test\_utils-0.0.43](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.43)

## [holochain\_cascade-0.0.44](crates/holochain_cascade/CHANGELOG.md#0.0.44)

## [holochain\_state-0.0.44](crates/holochain_state/CHANGELOG.md#0.0.44)

## [holochain\_p2p-0.0.42](crates/holochain_p2p/CHANGELOG.md#0.0.42)

## [holochain\_types-0.0.42](crates/holochain_types/CHANGELOG.md#0.0.42)

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `GlobalZomeTypes` type that holds all a dna’s zome types.
- `ToSqlStatement` trait for converting a type to a SQL statement.
- `InlineZomeSet` for creating a set of integrity and coordinator inline zomes.
- `DnaManifest` takes dependencies for coordinator zomes. These are the names of integrity zomes and must be within the same manifest.
- `DnaManifest` verifies that all zome names are unique.
- `DnaManifest` verifies that dependency names exists and are integrity zomes.
- `DnaFile` can hot swap coordinator zomes. Existing zomes are replaced and new zome names are appended.

### Changed

- `DnaStore` is now a `RibosomeStore`.
- `DnaManifest` now has an integrity key for all values that will change the dna hash.
- `DnaManifest` now has an optional coordinator key for adding coordinators zomes on install.

## [holochain\_keystore-0.0.42](crates/holochain_keystore/CHANGELOG.md#0.0.42)

## [holochain\_sqlite-0.0.41](crates/holochain_sqlite/CHANGELOG.md#0.0.41)

## [kitsune\_p2p-0.0.36](crates/kitsune_p2p/CHANGELOG.md#0.0.36)

## [mr\_bundle-0.0.13](crates/mr_bundle/CHANGELOG.md#0.0.13)

## [hdk-0.0.137](crates/hdk/CHANGELOG.md#0.0.137)

- hdk: Use newest wasmer and introduces `wasm_error!` macro to capture line numbers for wasm errors [\#1380](https://github.com/holochain/holochain/pull/1380)
- Docs: Restructure main page sections and add several intra-doc lnks [\#1418](https://github.com/holochain/holochain/pull/1418)
- hdk: Add functional stub for `x_salsa20_poly1305_shared_secret_create_random` [\#1410](https://github.com/holochain/holochain/pull/1410)
- hdk: Add functional stub for `x_salsa20_poly1305_shared_secret_export` [\#1410](https://github.com/holochain/holochain/pull/1410)
- hdk: Add functional stub for `x_salsa20_poly1305_shared_secret_ingest` [\#1410](https://github.com/holochain/holochain/pull/1410)
- Bump wasmer to 0.0.80 [\#1386](https://github.com/holochain/holochain/pull/1386)

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `get_links` and `get_link_details` take a `TryInto<LinkTypesRages>`. See the link test wasm for examples.

### Removed

- `entry_def_index` and `entry_type` macros are no longer needed.

### Changed

- `call` and `call_remote` now take an `Into<ZomeName>` instead of a `ZomeName`.
- `create_link` takes a `TryInto<LinkType>` instead of an `Into<LinkType>`.
- `update` takes `UpdateInput` instead of a `HeaderHash` and `CreateInput`.
- `create_entry` takes a type that can try into an `EntryDefIndex` and `EntryVisibility` instead of implementing `EntryDefRegistration`.
- `update_entry` takes the previous header hash and a try into `Entry` instead of a `EntryDefRegistration`.
- `Path` now must be `typed(LinkType)` to use any functionality that creates or gets links.

## [holochain\_zome\_types-0.0.36](crates/holochain_zome_types/CHANGELOG.md#0.0.36)

- Bump wasmer to 0.0.80 [\#1386](https://github.com/holochain/holochain/pull/1386)

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `ZomeDef` now holds dependencies for the zome.
- `EntryDefLocation` is either an `EntryDefIndex` or a `CapClaim` or a `CapGrant`.

### Changed

- Zomes are now generic over integrity and coordinator.

- `ZomeDef` is now wrapped in either `IntegrityZomeDef` or `CoordinatorZomeDef`.

- `GetLinksInput` takes a `LinkTypeRanges` for filtering on `LinkType`.

- `CreateInput` takes an `EntryDefLocation` for and an `EntryVisibility` for the entry.

- `UpdateInput` doesn’t take a `CreateInput` anymore.

- `UpdateInput` takes an `Entry` and `ChainTopOrdering`.

- `DnaDef` has split zomes into integrity and coordinator.

- `DnaDef` coordinator zomes do not change the `DnaHash`.

- Docs: Describe init callback and link to WASM examples [\#1418](https://github.com/holochain/holochain/pull/1418)

## [holochain\_deterministic\_integrity-0.0.9](crates/hdi/CHANGELOG.md#0.0.9)

- Bump wasmer to 0.0.80 [\#1386](https://github.com/holochain/holochain/pull/1386)

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `EntryTypesHelper` helper trait for deserializing to the correct `Entry`.
- `LinkTypesHelper` helper trait for creating `LinkTypeRanges` that fit the current local scope.

### Removed

- `register_entry!` macro as it is no longer needed. Use `hdk_derive::hdk_entry_defs`.

## [hdk\_derive-0.0.36](crates/hdk_derive/CHANGELOG.md#0.0.36)

## [holochain\_integrity\_types-0.0.8](crates/holochain_integrity_types/CHANGELOG.md#0.0.8)

- KeyRef (opaque reference to a secretbox shared secret) is now an unsized byte slice [\#1410](https://github.com/holochain/holochain/pull/1410)

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `ZomeInfo` now contains the `ScopedZomeTypesSet`. This is all the zome types that are in scope for the calling zome.
- `LinkTypeRanges` for are used querying of links.
- `ScopedZomeTypesSet` and `ScopedZomeTypes` for scoping between local and global zome types.
- `GlobalZomeTypeId` and `LocalZomeTypeId` for identifying zome types within different scopes.
- `UnitEnum` trait for associating an enum with non-unit variants with an equivalent unit variants.
- `EntryDefRegistration` for associating entry defs with entry types.

### Removed

- `EntryDefs::entry_def_index_from_id` is removed because it’s no longer possible to go from an `EntryDefId` to a `GlobalZomeTypeId` as `EntryDefId` is not globally unique.
- `ZomeInfo::matches_entry_def_id` for the same reason as `EntryDefs::entry_def_index_from_id`
- `require_validation_type` is removed because it is no longer used.
- `ZomeId` from `CreateLink` as it’s no longer needed because `LinkType` is a `GlobalZomeTypeId`.
- `ZomeId` from `AppEntryType` as it’s no longer needed because `EntryDefIndex` is a `GlobalZomeTypeId`

### Changed

- ZomeName is now a `Cow<'static, str>` instead of a `String`.

## [holo\_hash-0.0.28](crates/holo_hash/CHANGELOG.md#0.0.28)

## [fixt-0.0.12](crates/fixt/CHANGELOG.md#0.0.12)

# 20220608.011447

## [holochain\_cli-0.0.41](crates/holochain_cli/CHANGELOG.md#0.0.41)

## [holochain\_cli\_sandbox-0.0.37](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.37)

## [holochain\_cli\_bundle-0.0.37](crates/holochain_cli_bundle/CHANGELOG.md#0.0.37)

## [holochain-0.0.143](crates/holochain/CHANGELOG.md#0.0.143)

## [holochain\_test\_wasm\_common-0.0.37](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.37)

## [holochain\_conductor\_api-0.0.43](crates/holochain_conductor_api/CHANGELOG.md#0.0.43)

## [holochain\_wasm\_test\_utils-0.0.42](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.42)

## [holochain\_cascade-0.0.43](crates/holochain_cascade/CHANGELOG.md#0.0.43)

## [holochain\_state-0.0.43](crates/holochain_state/CHANGELOG.md#0.0.43)

## [holochain\_p2p-0.0.41](crates/holochain_p2p/CHANGELOG.md#0.0.41)

## [holochain\_types-0.0.41](crates/holochain_types/CHANGELOG.md#0.0.41)

## [holochain\_keystore-0.0.41](crates/holochain_keystore/CHANGELOG.md#0.0.41)

- Docs: Crate README generated from crate level doc comments [\#1392](https://github.com/holochain/holochain/pull/1392).

## [hdk-0.0.136](crates/hdk/CHANGELOG.md#0.0.136)

- Docs: Crate README generated from crate level doc comments [\#1392](https://github.com/holochain/holochain/pull/1392).

# 20220601.012853

## [holochain\_cli-0.0.40](crates/holochain_cli/CHANGELOG.md#0.0.40)

## [holochain\_cli\_sandbox-0.0.36](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.36)

## [holochain\_cli\_bundle-0.0.36](crates/holochain_cli_bundle/CHANGELOG.md#0.0.36)

## [holochain-0.0.142](crates/holochain/CHANGELOG.md#0.0.142)

## [holochain\_websocket-0.0.39](crates/holochain_websocket/CHANGELOG.md#0.0.39)

## [holochain\_test\_wasm\_common-0.0.36](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.36)

## [holochain\_conductor\_api-0.0.42](crates/holochain_conductor_api/CHANGELOG.md#0.0.42)

## [holochain\_wasm\_test\_utils-0.0.41](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.41)

## [holochain\_cascade-0.0.42](crates/holochain_cascade/CHANGELOG.md#0.0.42)

## [holochain\_state-0.0.42](crates/holochain_state/CHANGELOG.md#0.0.42)

## [holochain\_p2p-0.0.40](crates/holochain_p2p/CHANGELOG.md#0.0.40)

## [holochain\_types-0.0.40](crates/holochain_types/CHANGELOG.md#0.0.40)

## [holochain\_keystore-0.0.40](crates/holochain_keystore/CHANGELOG.md#0.0.40)

## [holochain\_sqlite-0.0.40](crates/holochain_sqlite/CHANGELOG.md#0.0.40)

## [kitsune\_p2p-0.0.35](crates/kitsune_p2p/CHANGELOG.md#0.0.35)

## [hdk-0.0.135](crates/hdk/CHANGELOG.md#0.0.135)

## [holochain\_zome\_types-0.0.35](crates/holochain_zome_types/CHANGELOG.md#0.0.35)

## [holochain\_deterministic\_integrity-0.0.8](crates/hdi/CHANGELOG.md#0.0.8)

## [hdk\_derive-0.0.35](crates/hdk_derive/CHANGELOG.md#0.0.35)

## [holochain\_integrity\_types-0.0.7](crates/holochain_integrity_types/CHANGELOG.md#0.0.7)

## [holo\_hash-0.0.27](crates/holo_hash/CHANGELOG.md#0.0.27)

# 20220525.012131

## [holochain\_cli-0.0.39](crates/holochain_cli/CHANGELOG.md#0.0.39)

## [holochain\_cli\_sandbox-0.0.35](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.35)

## [holochain\_cli\_bundle-0.0.35](crates/holochain_cli_bundle/CHANGELOG.md#0.0.35)

## [holochain-0.0.141](crates/holochain/CHANGELOG.md#0.0.141)

## [holochain\_test\_wasm\_common-0.0.35](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.35)

## [holochain\_conductor\_api-0.0.41](crates/holochain_conductor_api/CHANGELOG.md#0.0.41)

- Docs: Unify and clean up docs for admin and app interface and conductor config. [\#1391](https://github.com/holochain/holochain/pull/1391)

## [holochain\_wasm\_test\_utils-0.0.40](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.40)

## [holochain\_cascade-0.0.41](crates/holochain_cascade/CHANGELOG.md#0.0.41)

## [holochain\_state-0.0.41](crates/holochain_state/CHANGELOG.md#0.0.41)

## [holochain\_p2p-0.0.39](crates/holochain_p2p/CHANGELOG.md#0.0.39)

## [holochain\_types-0.0.39](crates/holochain_types/CHANGELOG.md#0.0.39)

## [holochain\_keystore-0.0.39](crates/holochain_keystore/CHANGELOG.md#0.0.39)

## [holochain\_sqlite-0.0.39](crates/holochain_sqlite/CHANGELOG.md#0.0.39)

## [kitsune\_p2p-0.0.34](crates/kitsune_p2p/CHANGELOG.md#0.0.34)

## [mr\_bundle-0.0.12](crates/mr_bundle/CHANGELOG.md#0.0.12)

## [hdk-0.0.134](crates/hdk/CHANGELOG.md#0.0.134)

## [holochain\_zome\_types-0.0.34](crates/holochain_zome_types/CHANGELOG.md#0.0.34)

## [holochain\_deterministic\_integrity-0.0.7](crates/hdi/CHANGELOG.md#0.0.7)

- Fix broken wasm tracing. [PR](https://github.com/holochain/holochain/pull/1389).

## [hdk\_derive-0.0.34](crates/hdk_derive/CHANGELOG.md#0.0.34)

## [holochain\_integrity\_types-0.0.6](crates/holochain_integrity_types/CHANGELOG.md#0.0.6)

## [holo\_hash-0.0.26](crates/holo_hash/CHANGELOG.md#0.0.26)

# 20220518.010753

## [holochain\_cli-0.0.38](crates/holochain_cli/CHANGELOG.md#0.0.38)

## [holochain\_cli\_sandbox-0.0.34](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.34)

## [holochain\_cli\_bundle-0.0.34](crates/holochain_cli_bundle/CHANGELOG.md#0.0.34)

## [holochain-0.0.140](crates/holochain/CHANGELOG.md#0.0.140)

## [holochain\_websocket-0.0.38](crates/holochain_websocket/CHANGELOG.md#0.0.38)

## [holochain\_test\_wasm\_common-0.0.34](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.34)

## [holochain\_conductor\_api-0.0.40](crates/holochain_conductor_api/CHANGELOG.md#0.0.40)

## [holochain\_wasm\_test\_utils-0.0.39](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.39)

## [holochain\_cascade-0.0.40](crates/holochain_cascade/CHANGELOG.md#0.0.40)

## [holochain\_state-0.0.40](crates/holochain_state/CHANGELOG.md#0.0.40)

## [holochain\_p2p-0.0.38](crates/holochain_p2p/CHANGELOG.md#0.0.38)

## [holochain\_types-0.0.38](crates/holochain_types/CHANGELOG.md#0.0.38)

## [holochain\_keystore-0.0.38](crates/holochain_keystore/CHANGELOG.md#0.0.38)

## [holochain\_sqlite-0.0.38](crates/holochain_sqlite/CHANGELOG.md#0.0.38)

## [kitsune\_p2p-0.0.33](crates/kitsune_p2p/CHANGELOG.md#0.0.33)

## [hdk-0.0.133](crates/hdk/CHANGELOG.md#0.0.133)

## [holochain\_deterministic\_integrity-0.0.6](crates/hdi/CHANGELOG.md#0.0.6)

# 20220511.012519

## [holochain\_cli-0.0.37](crates/holochain_cli/CHANGELOG.md#0.0.37)

## [holochain\_cli\_sandbox-0.0.33](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.33)

## [holochain\_cli\_bundle-0.0.33](crates/holochain_cli_bundle/CHANGELOG.md#0.0.33)

## [holochain-0.0.139](crates/holochain/CHANGELOG.md#0.0.139)

- Udpate lair to 0.1.3 - largely just documentation updates, but also re-introduces some dependency pinning to fix mismatch client/server version check [\#1377](https://github.com/holochain/holochain/pull/1377)

## [holochain\_websocket-0.0.37](crates/holochain_websocket/CHANGELOG.md#0.0.37)

## [holochain\_test\_wasm\_common-0.0.33](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.33)

## [holochain\_conductor\_api-0.0.39](crates/holochain_conductor_api/CHANGELOG.md#0.0.39)

## [holochain\_cascade-0.0.39](crates/holochain_cascade/CHANGELOG.md#0.0.39)

## [holochain\_state-0.0.39](crates/holochain_state/CHANGELOG.md#0.0.39)

## [holochain\_wasm\_test\_utils-0.0.38](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.38)

## [holochain\_p2p-0.0.37](crates/holochain_p2p/CHANGELOG.md#0.0.37)

## [kitsune\_p2p\_bootstrap-0.0.11](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.11)

## [holochain\_types-0.0.37](crates/holochain_types/CHANGELOG.md#0.0.37)

## [holochain\_keystore-0.0.37](crates/holochain_keystore/CHANGELOG.md#0.0.37)

## [holochain\_sqlite-0.0.37](crates/holochain_sqlite/CHANGELOG.md#0.0.37)

## [kitsune\_p2p-0.0.32](crates/kitsune_p2p/CHANGELOG.md#0.0.32)

## [kitsune\_p2p\_proxy-0.0.24](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.24)

## [kitsune\_p2p\_transport\_quic-0.0.24](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.24)

## [kitsune\_p2p\_types-0.0.24](crates/kitsune_p2p_types/CHANGELOG.md#0.0.24)

## [hdk-0.0.132](crates/hdk/CHANGELOG.md#0.0.132)

- hdk: Provide `Into<AnyLinkableHash>` impl for `EntryHash` and `HeaderHash`. This allows `create_link` and `get_links` to be used directly with EntryHash and HeaderHash arguments, rather than needing to construct an `AnyLinkableHash` explicitly.

## [holochain\_zome\_types-0.0.33](crates/holochain_zome_types/CHANGELOG.md#0.0.33)

## [holochain\_deterministic\_integrity-0.0.5](crates/hdi/CHANGELOG.md#0.0.5)

## [hdk\_derive-0.0.33](crates/hdk_derive/CHANGELOG.md#0.0.33)

## [holochain\_integrity\_types-0.0.5](crates/holochain_integrity_types/CHANGELOG.md#0.0.5)

## [holo\_hash-0.0.25](crates/holo_hash/CHANGELOG.md#0.0.25)

- Add `Into<AnyLinkableHash>` impl for `EntryHashB64` and `HeaderHashB64`
- Add some helpful methods for converting from a “composite” hash type (`AnyDhtHash` or `AnyLinkableHash`) into their respective primitive types:
  - `AnyDhtHash::into_primitive()`, returns an enum
  - `AnyDhtHash::into_entry_hash()`, returns `Option<EntryHash>`
  - `AnyDhtHash::into_header_hash()`, returns `Option<HeaderHash>`
  - `AnyLinkableHash::into_primitive()`, returns an enum
  - `AnyLinkableHash::into_entry_hash()`, returns `Option<EntryHash>`
  - `AnyLinkableHash::into_header_hash()`, returns `Option<HeaderHash>`
  - `AnyLinkableHash::into_external_hash()`, returns `Option<ExternalHash>`

# 20220505.103150

## [holochain\_cli-0.0.36](crates/holochain_cli/CHANGELOG.md#0.0.36)

## [holochain\_cli\_sandbox-0.0.32](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.32)

## [holochain-0.0.138](crates/holochain/CHANGELOG.md#0.0.138)

## [holochain\_conductor\_api-0.0.38](crates/holochain_conductor_api/CHANGELOG.md#0.0.38)

## [holochain\_cascade-0.0.38](crates/holochain_cascade/CHANGELOG.md#0.0.38)

## [holochain\_state-0.0.38](crates/holochain_state/CHANGELOG.md#0.0.38)

## [holochain\_wasm\_test\_utils-0.0.37](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.37)

# 20220429.205522

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

## [holochain\_cli-0.0.35](crates/holochain_cli/CHANGELOG.md#0.0.35)

## [holochain\_cli\_sandbox-0.0.31](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.31)

## [holochain\_cli\_bundle-0.0.32](crates/holochain_cli_bundle/CHANGELOG.md#0.0.32)

## [holochain-0.0.137](crates/holochain/CHANGELOG.md#0.0.137)

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)
- Update legacy lair to 0.0.10 - allowing “panicky” flag [\#1349](https://github.com/holochain/holochain/pull/1349)
- Udpate lair to 0.1.1 - allowing usage in path with whitespace [\#1349](https://github.com/holochain/holochain/pull/1349)

## [holochain\_websocket-0.0.36](crates/holochain_websocket/CHANGELOG.md#0.0.36)

## [holochain\_test\_wasm\_common-0.0.32](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.32)

## [holochain\_conductor\_api-0.0.37](crates/holochain_conductor_api/CHANGELOG.md#0.0.37)

- Docs: Fix intra-doc links in crates `holochain_conductor_api` and `holochain_state` [\#1323](https://github.com/holochain/holochain/pull/1323)

## [holochain\_cascade-0.0.37](crates/holochain_cascade/CHANGELOG.md#0.0.37)

## [holochain\_state-0.0.37](crates/holochain_state/CHANGELOG.md#0.0.37)

- Docs: Fix intra-doc links in crates `holochain_conductor_api` and `holochain_state` [\#1323](https://github.com/holochain/holochain/pull/1323)

## [holochain\_wasm\_test\_utils-0.0.36](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.36)

## [holochain\_p2p-0.0.36](crates/holochain_p2p/CHANGELOG.md#0.0.36)

## [kitsune\_p2p\_bootstrap-0.0.10](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.10)

## [holochain\_types-0.0.36](crates/holochain_types/CHANGELOG.md#0.0.36)

## [holochain\_keystore-0.0.36](crates/holochain_keystore/CHANGELOG.md#0.0.36)

## [holochain\_sqlite-0.0.36](crates/holochain_sqlite/CHANGELOG.md#0.0.36)

## [kitsune\_p2p-0.0.31](crates/kitsune_p2p/CHANGELOG.md#0.0.31)

## [kitsune\_p2p\_proxy-0.0.23](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.23)

## [kitsune\_p2p\_transport\_quic-0.0.23](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.23)

## [kitsune\_p2p\_types-0.0.23](crates/kitsune_p2p_types/CHANGELOG.md#0.0.23)

## [mr\_bundle-0.0.11](crates/mr_bundle/CHANGELOG.md#0.0.11)

## [holochain\_util-0.0.10](crates/holochain_util/CHANGELOG.md#0.0.10)

## [hdk-0.0.131](crates/hdk/CHANGELOG.md#0.0.131)

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

## [holochain\_zome\_types-0.0.32](crates/holochain_zome_types/CHANGELOG.md#0.0.32)

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

## [holochain\_deterministic\_integrity-0.0.4](crates/hdi/CHANGELOG.md#0.0.4)

## [hdk\_derive-0.0.32](crates/hdk_derive/CHANGELOG.md#0.0.32)

## [holochain\_integrity\_types-0.0.4](crates/holochain_integrity_types/CHANGELOG.md#0.0.4)

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

## [kitsune\_p2p\_timestamp-0.0.9](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.9)

## [holo\_hash-0.0.24](crates/holo_hash/CHANGELOG.md#0.0.24)

## [fixt-0.0.11](crates/fixt/CHANGELOG.md#0.0.11)

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

# 20220421.145237

## [holochain\_cli-0.0.34](crates/holochain_cli/CHANGELOG.md#0.0.34)

## [holochain\_cli\_sandbox-0.0.30](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.30)

## [holochain\_cli\_bundle-0.0.31](crates/holochain_cli_bundle/CHANGELOG.md#0.0.31)

## [holochain-0.0.136](crates/holochain/CHANGELOG.md#0.0.136)

## [holochain\_websocket-0.0.35](crates/holochain_websocket/CHANGELOG.md#0.0.35)

## [holochain\_test\_wasm\_common-0.0.31](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.31)

## [holochain\_conductor\_api-0.0.36](crates/holochain_conductor_api/CHANGELOG.md#0.0.36)

## [holochain\_cascade-0.0.36](crates/holochain_cascade/CHANGELOG.md#0.0.36)

## [holochain\_state-0.0.36](crates/holochain_state/CHANGELOG.md#0.0.36)

## [holochain\_wasm\_test\_utils-0.0.35](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.35)

## [holochain\_p2p-0.0.35](crates/holochain_p2p/CHANGELOG.md#0.0.35)

## [holochain\_types-0.0.35](crates/holochain_types/CHANGELOG.md#0.0.35)

## [holochain\_keystore-0.0.35](crates/holochain_keystore/CHANGELOG.md#0.0.35)

## [holochain\_sqlite-0.0.35](crates/holochain_sqlite/CHANGELOG.md#0.0.35)

## [hdk-0.0.130](crates/hdk/CHANGELOG.md#0.0.130)

## [holochain\_zome\_types-0.0.31](crates/holochain_zome_types/CHANGELOG.md#0.0.31)

## [holochain\_deterministic\_integrity-0.0.3](crates/hdi/CHANGELOG.md#0.0.3)

## [hdk\_derive-0.0.31](crates/hdk_derive/CHANGELOG.md#0.0.31)

## [holochain\_integrity\_types-0.0.3](crates/holochain_integrity_types/CHANGELOG.md#0.0.3)

# 20220414.075333

## [holochain\_cli-0.0.33](crates/holochain_cli/CHANGELOG.md#0.0.33)

## [holochain\_cli\_sandbox-0.0.29](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.29)

## [holochain-0.0.135](crates/holochain/CHANGELOG.md#0.0.135)

## [holochain\_test\_wasm\_common-0.0.30](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.30)

## [holochain\_conductor\_api-0.0.35](crates/holochain_conductor_api/CHANGELOG.md#0.0.35)

## [holochain\_cascade-0.0.35](crates/holochain_cascade/CHANGELOG.md#0.0.35)

## [holochain\_state-0.0.35](crates/holochain_state/CHANGELOG.md#0.0.35)

## [hdk-0.0.129](crates/hdk/CHANGELOG.md#0.0.129)

## [holochain\_deterministic\_integrity-0.0.2](crates/hdi/CHANGELOG.md#0.0.2)

# 20220413.011152

## [holochain\_cli-0.0.32](crates/holochain_cli/CHANGELOG.md#0.0.32)

- Fixed broken links in Rust docs [\#1284](https://github.com/holochain/holochain/pull/1284)

## [holochain\_cli\_sandbox-0.0.28](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.28)

- `hc sandbox` command for installing happs was limited to 16mb websocket message limit and would error if given a large happ bundle. now it won’t.  [\#1322](https://github.com/holochain/holochain/pull/1322)
- Fixed broken links in Rust docs [\#1284](https://github.com/holochain/holochain/pull/1284)

## [holochain\_cli\_bundle-0.0.30](crates/holochain_cli_bundle/CHANGELOG.md#0.0.30)

## [holochain-0.0.134](crates/holochain/CHANGELOG.md#0.0.134)

## [holochain\_websocket-0.0.34](crates/holochain_websocket/CHANGELOG.md#0.0.34)

## [holochain\_test\_wasm\_common-0.0.29](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.29)

## [holochain\_conductor\_api-0.0.34](crates/holochain_conductor_api/CHANGELOG.md#0.0.34)

## [holochain\_cascade-0.0.34](crates/holochain_cascade/CHANGELOG.md#0.0.34)

## [holochain\_state-0.0.34](crates/holochain_state/CHANGELOG.md#0.0.34)

## [holochain\_wasm\_test\_utils-0.0.34](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.34)

## [holochain\_p2p-0.0.34](crates/holochain_p2p/CHANGELOG.md#0.0.34)

## [kitsune\_p2p\_bootstrap-0.0.9](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.9)

## [holochain\_types-0.0.34](crates/holochain_types/CHANGELOG.md#0.0.34)

## [holochain\_keystore-0.0.34](crates/holochain_keystore/CHANGELOG.md#0.0.34)

## [holochain\_sqlite-0.0.34](crates/holochain_sqlite/CHANGELOG.md#0.0.34)

## [kitsune\_p2p-0.0.30](crates/kitsune_p2p/CHANGELOG.md#0.0.30)

## [hdk-0.0.128](crates/hdk/CHANGELOG.md#0.0.128)

- hdk: Adds external hash type for data that has a DHT location but does not exist on the DHT [\#1298](https://github.com/holochain/holochain/pull/1298)
- hdk: Adds compound hash type for linkable hashes [\#1308](https://github.com/holochain/holochain/pull/1308)
- hdk: Missing dependencies are fetched async for validation [\#1268](https://github.com/holochain/holochain/pull/1268)

## [holochain\_zome\_types-0.0.30](crates/holochain_zome_types/CHANGELOG.md#0.0.30)

## [holochain\_deterministic\_integrity-0.0.1](crates/hdi/CHANGELOG.md#0.0.1)

## [hdk\_derive-0.0.30](crates/hdk_derive/CHANGELOG.md#0.0.30)

## [holochain\_integrity\_types-0.0.2](crates/holochain_integrity_types/CHANGELOG.md#0.0.2)

# 20220406.010602

## [holochain\_cli\_bundle-0.0.29](crates/holochain_cli_bundle/CHANGELOG.md#0.0.29)

## [holochain-0.0.133](crates/holochain/CHANGELOG.md#0.0.133)

## [holochain\_websocket-0.0.33](crates/holochain_websocket/CHANGELOG.md#0.0.33)

## [holochain\_test\_wasm\_common-0.0.28](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.28)

## [holochain\_conductor\_api-0.0.33](crates/holochain_conductor_api/CHANGELOG.md#0.0.33)

## [holochain\_cascade-0.0.33](crates/holochain_cascade/CHANGELOG.md#0.0.33)

## [holochain\_state-0.0.33](crates/holochain_state/CHANGELOG.md#0.0.33)

## [holochain\_wasm\_test\_utils-0.0.33](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.33)

## [holochain\_p2p-0.0.33](crates/holochain_p2p/CHANGELOG.md#0.0.33)

## [kitsune\_p2p\_bootstrap-0.0.8](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.8)

## [holochain\_types-0.0.33](crates/holochain_types/CHANGELOG.md#0.0.33)

## [holochain\_keystore-0.0.33](crates/holochain_keystore/CHANGELOG.md#0.0.33)

## [holochain\_sqlite-0.0.33](crates/holochain_sqlite/CHANGELOG.md#0.0.33)

## [kitsune\_p2p-0.0.29](crates/kitsune_p2p/CHANGELOG.md#0.0.29)

## [mr\_bundle-0.0.10](crates/mr_bundle/CHANGELOG.md#0.0.10)

## [hdk\_derive-0.0.29](crates/hdk_derive/CHANGELOG.md#0.0.29)

## [holochain\_zome\_types-0.0.29](crates/holochain_zome_types/CHANGELOG.md#0.0.29)

## [holochain\_integrity\_types-0.0.1](crates/holochain_integrity_types/CHANGELOG.md#0.0.1)

## [kitsune\_p2p\_timestamp-0.0.8](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.8)

- **BREAKING**: All chrono logic is behind the `chrono` feature flag which is on by default. If you are using this crate with `no-default-features` you will no longer have access to any chrono related functionality.

## [holo\_hash-0.0.23](crates/holo_hash/CHANGELOG.md#0.0.23)

# 20220330.010719

## [holochain-0.0.132](crates/holochain/CHANGELOG.md#0.0.132)

## [holochain\_test\_wasm\_common-0.0.27](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.27)

## [holochain\_cascade-0.0.32](crates/holochain_cascade/CHANGELOG.md#0.0.32)

## [holochain\_websocket-0.0.32](crates/holochain_websocket/CHANGELOG.md#0.0.32)

## [holochain\_conductor\_api-0.0.32](crates/holochain_conductor_api/CHANGELOG.md#0.0.32)

## [holochain\_state-0.0.32](crates/holochain_state/CHANGELOG.md#0.0.32)

## [holochain\_wasm\_test\_utils-0.0.32](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.32)

## [holochain\_p2p-0.0.32](crates/holochain_p2p/CHANGELOG.md#0.0.32)

## [kitsune\_p2p\_bootstrap-0.0.7](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.7)

## [holochain\_cli\_bundle-0.0.28](crates/holochain_cli_bundle/CHANGELOG.md#0.0.28)

## [holochain\_types-0.0.32](crates/holochain_types/CHANGELOG.md#0.0.32)

## [holochain\_keystore-0.0.32](crates/holochain_keystore/CHANGELOG.md#0.0.32)

## [holochain\_sqlite-0.0.32](crates/holochain_sqlite/CHANGELOG.md#0.0.32)

## [kitsune\_p2p-0.0.28](crates/kitsune_p2p/CHANGELOG.md#0.0.28)

## [kitsune\_p2p\_proxy-0.0.22](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.22)

## [kitsune\_p2p\_transport\_quic-0.0.22](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.22)

## [kitsune\_p2p\_types-0.0.22](crates/kitsune_p2p_types/CHANGELOG.md#0.0.22)

## [hdk-0.0.127](crates/hdk/CHANGELOG.md#0.0.127)

## [hdk\_derive-0.0.28](crates/hdk_derive/CHANGELOG.md#0.0.28)

## [holochain\_zome\_types-0.0.28](crates/holochain_zome_types/CHANGELOG.md#0.0.28)

## [holo\_hash-0.0.22](crates/holo_hash/CHANGELOG.md#0.0.22)

## [kitsune\_p2p\_dht\_arc-0.0.11](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.11)

- **BREAKING** Arcs are now “unidirectional”, meaning rather than the agent location defining the centerpoint of the storage arc, the agent location defines the left edge of the arc.

This is a huge change, particularly to gossip behavior. With bidirectional arcs, when peers have roughly equivalently sized arcs, half of the peers who have overlapping arcs will not see each other or gossip with each other because their centerpoints are not contained within each others’ arcs. With unidirectional arcs, this problem is removed at the expense of making peer discovery asymmmetrical, which we have found to have no adverse effects.

## [fixt-0.0.10](crates/fixt/CHANGELOG.md#0.0.10)

# 20220323.023956

## [holochain-0.0.131](crates/holochain/CHANGELOG.md#0.0.131)

- When joining the network set arc size to previous value if available instead of full to avoid network load [1287](https://github.com/holochain/holochain/pull/1287)

## [holochain\_test\_wasm\_common-0.0.26](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.26)

## [holochain\_cascade-0.0.31](crates/holochain_cascade/CHANGELOG.md#0.0.31)

## [holochain\_websocket-0.0.31](crates/holochain_websocket/CHANGELOG.md#0.0.31)

## [holochain\_conductor\_api-0.0.31](crates/holochain_conductor_api/CHANGELOG.md#0.0.31)

## [holochain\_state-0.0.31](crates/holochain_state/CHANGELOG.md#0.0.31)

## [holochain\_wasm\_test\_utils-0.0.31](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.31)

## [holochain\_p2p-0.0.31](crates/holochain_p2p/CHANGELOG.md#0.0.31)

## [kitsune\_p2p\_bootstrap-0.0.6](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.6)

## [holochain\_cli\_bundle-0.0.27](crates/holochain_cli_bundle/CHANGELOG.md#0.0.27)

## [holochain\_types-0.0.31](crates/holochain_types/CHANGELOG.md#0.0.31)

## [holochain\_keystore-0.0.31](crates/holochain_keystore/CHANGELOG.md#0.0.31)

## [holochain\_sqlite-0.0.31](crates/holochain_sqlite/CHANGELOG.md#0.0.31)

## [kitsune\_p2p-0.0.27](crates/kitsune_p2p/CHANGELOG.md#0.0.27)

## [kitsune\_p2p\_proxy-0.0.21](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.21)

## [kitsune\_p2p\_transport\_quic-0.0.21](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.21)

## [kitsune\_p2p\_types-0.0.21](crates/kitsune_p2p_types/CHANGELOG.md#0.0.21)

## [mr\_bundle-0.0.9](crates/mr_bundle/CHANGELOG.md#0.0.9)

## [holochain\_util-0.0.9](crates/holochain_util/CHANGELOG.md#0.0.9)

## [hdk-0.0.126](crates/hdk/CHANGELOG.md#0.0.126)

- Docs: Explain how hashes in Holochain are composed and its components on the module page for `hdk::hash` [\#1299](https://github.com/holochain/holochain/pull/1299).

# 20220316.022611

## [holochain-0.0.130](crates/holochain/CHANGELOG.md#0.0.130)

- Workflow errors generally now log rather than abort the current app [1279](https://github.com/holochain/holochain/pull/1279/files)

## [holochain\_test\_wasm\_common-0.0.25](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.25)

## [holochain\_cascade-0.0.30](crates/holochain_cascade/CHANGELOG.md#0.0.30)

## [holochain\_cli-0.0.31](crates/holochain_cli/CHANGELOG.md#0.0.31)

## [holochain\_cli\_sandbox-0.0.27](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.27)

## [holochain\_websocket-0.0.30](crates/holochain_websocket/CHANGELOG.md#0.0.30)

## [holochain\_conductor\_api-0.0.30](crates/holochain_conductor_api/CHANGELOG.md#0.0.30)

## [holochain\_state-0.0.30](crates/holochain_state/CHANGELOG.md#0.0.30)

## [holochain\_wasm\_test\_utils-0.0.30](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.30)

## [holochain\_p2p-0.0.30](crates/holochain_p2p/CHANGELOG.md#0.0.30)

## [holochain\_cli\_bundle-0.0.26](crates/holochain_cli_bundle/CHANGELOG.md#0.0.26)

## [holochain\_types-0.0.30](crates/holochain_types/CHANGELOG.md#0.0.30)

## [holochain\_keystore-0.0.30](crates/holochain_keystore/CHANGELOG.md#0.0.30)

## [holochain\_sqlite-0.0.30](crates/holochain_sqlite/CHANGELOG.md#0.0.30)

## [hdk-0.0.125](crates/hdk/CHANGELOG.md#0.0.125)

- hdk: link base and target are no longer required to exist on the current DHT and aren’t made available via. validation ops (use must\_get\_entry instead) [\#1266](https://github.com/holochain/holochain/pull/1266)

## [hdk\_derive-0.0.27](crates/hdk_derive/CHANGELOG.md#0.0.27)

## [holochain\_zome\_types-0.0.27](crates/holochain_zome_types/CHANGELOG.md#0.0.27)

# 20220309.134939

## [holochain-0.0.129](crates/holochain/CHANGELOG.md#0.0.129)

## [holochain\_cascade-0.0.29](crates/holochain_cascade/CHANGELOG.md#0.0.29)

## [holochain\_cli-0.0.30](crates/holochain_cli/CHANGELOG.md#0.0.30)

## [holochain\_cli\_sandbox-0.0.26](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.26)

## [holochain\_websocket-0.0.29](crates/holochain_websocket/CHANGELOG.md#0.0.29)

## [holochain\_conductor\_api-0.0.29](crates/holochain_conductor_api/CHANGELOG.md#0.0.29)

## [holochain\_state-0.0.29](crates/holochain_state/CHANGELOG.md#0.0.29)

## [holochain\_wasm\_test\_utils-0.0.29](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.29)

## [holochain\_p2p-0.0.29](crates/holochain_p2p/CHANGELOG.md#0.0.29)

## [kitsune\_p2p\_bootstrap-0.0.5](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.5)

## [holochain\_cli\_bundle-0.0.25](crates/holochain_cli_bundle/CHANGELOG.md#0.0.25)

## [holochain\_types-0.0.29](crates/holochain_types/CHANGELOG.md#0.0.29)

## [holochain\_keystore-0.0.29](crates/holochain_keystore/CHANGELOG.md#0.0.29)

## [holochain\_sqlite-0.0.29](crates/holochain_sqlite/CHANGELOG.md#0.0.29)

## [kitsune\_p2p-0.0.26](crates/kitsune_p2p/CHANGELOG.md#0.0.26)

- Allow TLS session keylogging via tuning param `danger_tls_keylog` = `env_keylog`, and environment variable `SSLKEYLOGFILE` (See kitsune\_p2p crate api documentation). [\#1261](https://github.com/holochain/holochain/pull/1261)

## [kitsune\_p2p\_proxy-0.0.20](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.20)

## [kitsune\_p2p\_transport\_quic-0.0.20](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.20)

## [kitsune\_p2p\_types-0.0.20](crates/kitsune_p2p_types/CHANGELOG.md#0.0.20)

# 20220303.215755

## [holochain-0.0.128](crates/holochain/CHANGELOG.md#0.0.128)

- Proxy server chosen from bootstrap server proxy\_list [1242](https://github.com/holochain/holochain/pull/1242)

<!-- end list -->

``` yaml
network:
  transport_pool:
    - type: proxy
      proxy_config:
        type: remote_proxy_client_from_bootstrap
        bootstrap_url: https://bootstrap.holo.host
        fallback_proxy_url: ~
```

## [holochain\_test\_wasm\_common-0.0.24](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.24)

## [holochain\_cascade-0.0.28](crates/holochain_cascade/CHANGELOG.md#0.0.28)

## [holochain\_cli\_sandbox-0.0.25](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.25)

## [holochain\_websocket-0.0.28](crates/holochain_websocket/CHANGELOG.md#0.0.28)

## [holochain\_conductor\_api-0.0.28](crates/holochain_conductor_api/CHANGELOG.md#0.0.28)

## [holochain\_state-0.0.28](crates/holochain_state/CHANGELOG.md#0.0.28)

## [holochain\_wasm\_test\_utils-0.0.28](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.28)

## [holochain\_p2p-0.0.28](crates/holochain_p2p/CHANGELOG.md#0.0.28)

## [kitsune\_p2p\_bootstrap-0.0.4](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.4)

## [holochain\_cli-0.0.29](crates/holochain_cli/CHANGELOG.md#0.0.29)

## [holochain\_cli\_bundle-0.0.24](crates/holochain_cli_bundle/CHANGELOG.md#0.0.24)

## [holochain\_types-0.0.28](crates/holochain_types/CHANGELOG.md#0.0.28)

## [holochain\_keystore-0.0.28](crates/holochain_keystore/CHANGELOG.md#0.0.28)

## [holochain\_sqlite-0.0.28](crates/holochain_sqlite/CHANGELOG.md#0.0.28)

## [kitsune\_p2p-0.0.25](crates/kitsune_p2p/CHANGELOG.md#0.0.25)

- BREAKING: Gossip messages no longer contain the hash of the ops being gossiped. This is a breaking protocol change.
- Removed the unmaintained “simple-bloom” gossip module in favor of “sharded-gossip”

## [kitsune\_p2p\_proxy-0.0.19](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.19)

## [kitsune\_p2p\_transport\_quic-0.0.19](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.19)

## [kitsune\_p2p\_types-0.0.19](crates/kitsune_p2p_types/CHANGELOG.md#0.0.19)

## [kitsune\_p2p\_mdns-0.0.3](crates/kitsune_p2p_mdns/CHANGELOG.md#0.0.3)

## [mr\_bundle-0.0.8](crates/mr_bundle/CHANGELOG.md#0.0.8)

## [holochain\_util-0.0.8](crates/holochain_util/CHANGELOG.md#0.0.8)

## [hdk-0.0.124](crates/hdk/CHANGELOG.md#0.0.124)

## [hdk\_derive-0.0.26](crates/hdk_derive/CHANGELOG.md#0.0.26)

## [holochain\_zome\_types-0.0.26](crates/holochain_zome_types/CHANGELOG.md#0.0.26)

## [kitsune\_p2p\_timestamp-0.0.7](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.7)

## [holo\_hash-0.0.21](crates/holo_hash/CHANGELOG.md#0.0.21)

## [kitsune\_p2p\_dht\_arc-0.0.10](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.10)

## [fixt-0.0.9](crates/fixt/CHANGELOG.md#0.0.9)

# 20220223.090000

## [holochain-0.0.127](crates/holochain/CHANGELOG.md#0.0.127)

- **BREAKING CHANGE** App validation callbacks are now run per `Op`. There is now only a single validation callback `fn validate(op: Op) -> ExternResult<ValidateCallbackResult>` that is called for each `Op`. See the documentation for `Op` for more details on what data is passed to the callback. There are example use cases in `crates/test_utils/wasm/wasm_workspace/`. For example in the `validate` test wasm. To update an existing app, you to this version all `validate_*` callbacks including `validate_create_link` must be changed to the new `validate(..)` callback. [\#1212](https://github.com/holochain/holochain/pull/1212).

- `RegisterAgentActivity` ops are now validated by app validation.

- Init functions can now make zome calls. [\#1186](https://github.com/holochain/holochain/pull/1186)

- Adds header hashing to `hash` host fn [1227](https://github.com/holochain/holochain/pull/1227)

- Adds blake2b hashing to `hash` host fn [1228](https://github.com/holochain/holochain/pull/1228)

## [kitsune\_p2p\_bootstrap-0.0.3](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.3)

## [holochain\_test\_wasm\_common-0.0.23](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.23)

## [holochain\_cascade-0.0.27](crates/holochain_cascade/CHANGELOG.md#0.0.27)

## [holochain\_cli-0.0.28](crates/holochain_cli/CHANGELOG.md#0.0.28)

## [holochain\_cli\_sandbox-0.0.24](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.24)

## [holochain\_websocket-0.0.27](crates/holochain_websocket/CHANGELOG.md#0.0.27)

## [holochain\_conductor\_api-0.0.27](crates/holochain_conductor_api/CHANGELOG.md#0.0.27)

## [holochain\_state-0.0.27](crates/holochain_state/CHANGELOG.md#0.0.27)

## [holochain\_wasm\_test\_utils-0.0.27](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.27)

## [holochain\_p2p-0.0.27](crates/holochain_p2p/CHANGELOG.md#0.0.27)

## [holochain\_cli\_bundle-0.0.23](crates/holochain_cli_bundle/CHANGELOG.md#0.0.23)

- The DNA manifest now requires an `origin_time` Timestamp field, which will be used in the forthcoming gossip optimization.
  - There is a new system validation rule that all Header timestamps (including the initial Dna header) must come after the DNA’s `origin_time` field.
  - `hc dna init` injects the current system time as *microseconds* for the `origin_time` field of the DNA manifest
  - Since this field is not actually hooked up to anything at the moment, if the field is not present in a DNA manifest, a default `origin_time` of `January 1, 2022 12:00:00 AM` will be used instead. Once the new gossip algorithm lands, this default will be removed, and this will become a breaking change for DNA manifests which have not yet added an `origin_time`.

## [holochain\_types-0.0.27](crates/holochain_types/CHANGELOG.md#0.0.27)

## [holochain\_keystore-0.0.27](crates/holochain_keystore/CHANGELOG.md#0.0.27)

## [holochain\_sqlite-0.0.27](crates/holochain_sqlite/CHANGELOG.md#0.0.27)

## [kitsune\_p2p-0.0.24](crates/kitsune_p2p/CHANGELOG.md#0.0.24)

## [kitsune\_p2p\_proxy-0.0.18](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.18)

## [kitsune\_p2p\_transport\_quic-0.0.18](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.18)

## [kitsune\_p2p\_types-0.0.18](crates/kitsune_p2p_types/CHANGELOG.md#0.0.18)

- Sharded DHT arcs is on by default. This means that once the network reaches a certain size, it will split into multiple shards.

## [hdk-0.0.123](crates/hdk/CHANGELOG.md#0.0.123)

## [hdk\_derive-0.0.25](crates/hdk_derive/CHANGELOG.md#0.0.25)

## [holochain\_zome\_types-0.0.25](crates/holochain_zome_types/CHANGELOG.md#0.0.25)

- Adds the `Op` type which is used in the validation callback. [\#1212](https://github.com/holochain/holochain/pull/1212)
- Adds the `SignedHashed<T>` type for any data that can be signed and hashed.
- BREAKING CHANGE: Many hashing algorithms can now be specified although only the `Entry` hash type does anything yet [\#1222](https://github.com/holochain/holochain/pull/1222)

## [kitsune\_p2p\_timestamp-0.0.6](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.6)

## [holo\_hash-0.0.20](crates/holo_hash/CHANGELOG.md#0.0.20)

# 20220211.091841

- Bump `holochain_wasmer_*` crates to v0.0.77 which relaxes the version requirements on `serde`. [\#1204](https://github.com/holochain/holochain/pull/1204)

## [holochain-0.0.126](crates/holochain/CHANGELOG.md#0.0.126)

## [kitsune\_p2p\_bootstrap-0.0.2](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.2)

## [holochain\_test\_wasm\_common-0.0.22](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.22)

## [holochain\_cascade-0.0.26](crates/holochain_cascade/CHANGELOG.md#0.0.26)

## [holochain\_cli-0.0.27](crates/holochain_cli/CHANGELOG.md#0.0.27)

## [holochain\_cli\_sandbox-0.0.23](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.23)

## [holochain\_websocket-0.0.26](crates/holochain_websocket/CHANGELOG.md#0.0.26)

## [holochain\_conductor\_api-0.0.26](crates/holochain_conductor_api/CHANGELOG.md#0.0.26)

## [holochain\_state-0.0.26](crates/holochain_state/CHANGELOG.md#0.0.26)

## [holochain\_wasm\_test\_utils-0.0.26](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.26)

## [holochain\_p2p-0.0.26](crates/holochain_p2p/CHANGELOG.md#0.0.26)

## [holochain\_cli\_bundle-0.0.22](crates/holochain_cli_bundle/CHANGELOG.md#0.0.22)

## [holochain\_types-0.0.26](crates/holochain_types/CHANGELOG.md#0.0.26)

## [holochain\_keystore-0.0.26](crates/holochain_keystore/CHANGELOG.md#0.0.26)

## [holochain\_sqlite-0.0.26](crates/holochain_sqlite/CHANGELOG.md#0.0.26)

## [kitsune\_p2p-0.0.23](crates/kitsune_p2p/CHANGELOG.md#0.0.23)

- Fixes D-01415 holochain panic on startup [\#1206](https://github.com/holochain/holochain/pull/1206)

## [kitsune\_p2p\_proxy-0.0.17](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.17)

## [kitsune\_p2p\_transport\_quic-0.0.17](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.17)

## [kitsune\_p2p\_types-0.0.17](crates/kitsune_p2p_types/CHANGELOG.md#0.0.17)

## [mr\_bundle-0.0.7](crates/mr_bundle/CHANGELOG.md#0.0.7)

## [holochain\_util-0.0.7](crates/holochain_util/CHANGELOG.md#0.0.7)

## [hdk-0.0.122](crates/hdk/CHANGELOG.md#0.0.122)

- hdk: `delete`, `delete_entry`, and `delete_cap_grant` can all now take a `DeleteInput` as an argument to be able specify `ChainTopOrdering`, congruent with `create` and `update`. This change is backward compatible: a plain `HeaderHash` can still be used as input to `delete`.

## [hdk\_derive-0.0.24](crates/hdk_derive/CHANGELOG.md#0.0.24)

## [holochain\_zome\_types-0.0.24](crates/holochain_zome_types/CHANGELOG.md#0.0.24)

## [holo\_hash-0.0.19](crates/holo_hash/CHANGELOG.md#0.0.19)

## [kitsune\_p2p\_dht\_arc-0.0.9](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.9)

# 20220202.112225

## [holochain-0.0.125](crates/holochain/CHANGELOG.md#0.0.125)

## [kitsune\_p2p\_bootstrap-0.0.1](crates/kitsune_p2p_bootstrap/CHANGELOG.md#0.0.1)

## [holochain\_test\_wasm\_common-0.0.21](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.21)

## [holochain\_cascade-0.0.25](crates/holochain_cascade/CHANGELOG.md#0.0.25)

## [holochain\_cli-0.0.26](crates/holochain_cli/CHANGELOG.md#0.0.26)

## [holochain\_cli\_sandbox-0.0.22](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.22)

## [holochain\_websocket-0.0.25](crates/holochain_websocket/CHANGELOG.md#0.0.25)

## [holochain\_conductor\_api-0.0.25](crates/holochain_conductor_api/CHANGELOG.md#0.0.25)

## [holochain\_state-0.0.25](crates/holochain_state/CHANGELOG.md#0.0.25)

## [holochain\_wasm\_test\_utils-0.0.25](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.25)

## [holochain\_p2p-0.0.25](crates/holochain_p2p/CHANGELOG.md#0.0.25)

## [holochain\_cli\_bundle-0.0.21](crates/holochain_cli_bundle/CHANGELOG.md#0.0.21)

## [holochain\_types-0.0.25](crates/holochain_types/CHANGELOG.md#0.0.25)

## [holochain\_keystore-0.0.25](crates/holochain_keystore/CHANGELOG.md#0.0.25)

## [holochain\_sqlite-0.0.25](crates/holochain_sqlite/CHANGELOG.md#0.0.25)

## [kitsune\_p2p-0.0.22](crates/kitsune_p2p/CHANGELOG.md#0.0.22)

## [kitsune\_p2p\_proxy-0.0.16](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.16)

## [kitsune\_p2p\_transport\_quic-0.0.16](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.16)

## [kitsune\_p2p\_types-0.0.16](crates/kitsune_p2p_types/CHANGELOG.md#0.0.16)

## [hdk-0.0.121](crates/hdk/CHANGELOG.md#0.0.121)

## [hdk\_derive-0.0.23](crates/hdk_derive/CHANGELOG.md#0.0.23)

## [holochain\_zome\_types-0.0.23](crates/holochain_zome_types/CHANGELOG.md#0.0.23)

## [holo\_hash-0.0.18](crates/holo_hash/CHANGELOG.md#0.0.18)

## [kitsune\_p2p\_dht\_arc-0.0.8](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.8)

- New arc resizing algorithm based on `PeerViewBeta`
- In both arc resizing algorithms, instead of aiming for the ideal target arc size, aim for an ideal range. This slack in the system allows all agents to converge on their target more stably, with less oscillation.

# 20220126.200716

- Bump holochain-wasmer to fix a compilation issue. [\#1194](https://github.com/holochain/holochain/pull/1194)

## [holochain-0.0.124](crates/holochain/CHANGELOG.md#0.0.124)

## [holochain\_test\_wasm\_common-0.0.20](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.20)

## [holochain\_cascade-0.0.24](crates/holochain_cascade/CHANGELOG.md#0.0.24)

## [holochain\_cli-0.0.25](crates/holochain_cli/CHANGELOG.md#0.0.25)

## [holochain\_cli\_sandbox-0.0.21](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.21)

## [holochain\_websocket-0.0.24](crates/holochain_websocket/CHANGELOG.md#0.0.24)

## [holochain\_conductor\_api-0.0.24](crates/holochain_conductor_api/CHANGELOG.md#0.0.24)

## [holochain\_state-0.0.24](crates/holochain_state/CHANGELOG.md#0.0.24)

## [holochain\_wasm\_test\_utils-0.0.24](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.24)

## [holochain\_p2p-0.0.24](crates/holochain_p2p/CHANGELOG.md#0.0.24)

## [holochain\_cli\_bundle-0.0.20](crates/holochain_cli_bundle/CHANGELOG.md#0.0.20)

## [holochain\_types-0.0.24](crates/holochain_types/CHANGELOG.md#0.0.24)

## [holochain\_keystore-0.0.24](crates/holochain_keystore/CHANGELOG.md#0.0.24)

## [holochain\_sqlite-0.0.24](crates/holochain_sqlite/CHANGELOG.md#0.0.24)

## [kitsune\_p2p-0.0.21](crates/kitsune_p2p/CHANGELOG.md#0.0.21)

## [hdk-0.0.120](crates/hdk/CHANGELOG.md#0.0.120)

- docs: Add introduction to front-page and move example section up [1172](https://github.com/holochain/holochain/pull/1172)

## [hdk\_derive-0.0.22](crates/hdk_derive/CHANGELOG.md#0.0.22)

## [holochain\_zome\_types-0.0.22](crates/holochain_zome_types/CHANGELOG.md#0.0.22)

## [holo\_hash-0.0.17](crates/holo_hash/CHANGELOG.md#0.0.17)

# 20220120.093525

## [holochain-0.0.123](crates/holochain/CHANGELOG.md#0.0.123)

- Fixes issue where holochain could get stuck in infinite loop when trying to send validation receipts. [\#1181](https://github.com/holochain/holochain/pull/1181).
- Additional networking metric collection and associated admin api `DumpNetworkMetrics { dna_hash: Option<DnaHash> }` for inspection of metrics [\#1160](https://github.com/holochain/holochain/pull/1160)
- **BREAKING CHANGE** - Schema change for metrics database. Holochain will persist historical metrics once per hour, if you do not clear the metrics database it will crash at that point. [\#1183](https://github.com/holochain/holochain/pull/1183)

## [holochain\_test\_wasm\_common-0.0.19](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.19)

## [holochain\_cascade-0.0.23](crates/holochain_cascade/CHANGELOG.md#0.0.23)

## [holochain\_cli-0.0.24](crates/holochain_cli/CHANGELOG.md#0.0.24)

## [holochain\_cli\_sandbox-0.0.20](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.20)

## [holochain\_websocket-0.0.23](crates/holochain_websocket/CHANGELOG.md#0.0.23)

## [holochain\_conductor\_api-0.0.23](crates/holochain_conductor_api/CHANGELOG.md#0.0.23)

## [holochain\_state-0.0.23](crates/holochain_state/CHANGELOG.md#0.0.23)

## [holochain\_wasm\_test\_utils-0.0.23](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.23)

## [holochain\_p2p-0.0.23](crates/holochain_p2p/CHANGELOG.md#0.0.23)

## [holochain\_cli\_bundle-0.0.19](crates/holochain_cli_bundle/CHANGELOG.md#0.0.19)

## [holochain\_types-0.0.23](crates/holochain_types/CHANGELOG.md#0.0.23)

## [holochain\_keystore-0.0.23](crates/holochain_keystore/CHANGELOG.md#0.0.23)

## [holochain\_sqlite-0.0.23](crates/holochain_sqlite/CHANGELOG.md#0.0.23)

## [kitsune\_p2p-0.0.20](crates/kitsune_p2p/CHANGELOG.md#0.0.20)

## [hdk-0.0.119](crates/hdk/CHANGELOG.md#0.0.119)

## [hdk\_derive-0.0.21](crates/hdk_derive/CHANGELOG.md#0.0.21)

## [holochain\_zome\_types-0.0.21](crates/holochain_zome_types/CHANGELOG.md#0.0.21)

## [holo\_hash-0.0.16](crates/holo_hash/CHANGELOG.md#0.0.16)

# 20220106.093622

## [holochain-0.0.122](crates/holochain/CHANGELOG.md#0.0.122)

- Adds better batching to validation workflows for much faster validation. [\#1167](https://github.com/holochain/holochain/pull/1167).

## [holochain\_test\_wasm\_common-0.0.18](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.18)

## [holochain\_cascade-0.0.22](crates/holochain_cascade/CHANGELOG.md#0.0.22)

## [holochain\_cli-0.0.23](crates/holochain_cli/CHANGELOG.md#0.0.23)

## [holochain\_cli\_sandbox-0.0.19](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.19)

## [holochain\_websocket-0.0.22](crates/holochain_websocket/CHANGELOG.md#0.0.22)

## [holochain\_conductor\_api-0.0.22](crates/holochain_conductor_api/CHANGELOG.md#0.0.22)

- Adds the ability to manually insert elements into a source chain using the `AdminRequest::AddElements` command. Please check the docs and PR for more details / warnings on proper usage. [\#1166](https://github.com/holochain/holochain/pull/1166)

## [holochain\_state-0.0.22](crates/holochain_state/CHANGELOG.md#0.0.22)

## [holochain\_wasm\_test\_utils-0.0.22](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.22)

## [holochain\_p2p-0.0.22](crates/holochain_p2p/CHANGELOG.md#0.0.22)

## [holochain\_cli\_bundle-0.0.18](crates/holochain_cli_bundle/CHANGELOG.md#0.0.18)

## [holochain\_types-0.0.22](crates/holochain_types/CHANGELOG.md#0.0.22)

## [holochain\_keystore-0.0.22](crates/holochain_keystore/CHANGELOG.md#0.0.22)

## [holochain\_sqlite-0.0.22](crates/holochain_sqlite/CHANGELOG.md#0.0.22)

## [kitsune\_p2p-0.0.19](crates/kitsune_p2p/CHANGELOG.md#0.0.19)

## [hdk-0.0.118](crates/hdk/CHANGELOG.md#0.0.118)

- hdk: `Path` now split into `Path` and `PathEntry` [1156](https://github.com/holochain/holochain/pull/1156)
- hdk: Minor changes and additions to `Path` methods [1156](https://github.com/holochain/holochain/pull/1156)

## [hdk\_derive-0.0.20](crates/hdk_derive/CHANGELOG.md#0.0.20)

## [holochain\_zome\_types-0.0.20](crates/holochain_zome_types/CHANGELOG.md#0.0.20)

- BREAKING CHANGE: Range filters on chain queries are now INCLUSIVE and support hash bounds [\#1142](https://github.com/holochain/holochain/pull/1142)
- BREAKING CHANGE: Chain queries now support restricting results to a list of entry hashes [\#1142](https://github.com/holochain/holochain/pull/1142)

## [holo\_hash-0.0.15](crates/holo_hash/CHANGELOG.md#0.0.15)

# 20211222.094252

## [holochain-0.0.121](crates/holochain/CHANGELOG.md#0.0.121)

- **BREAKING CHANGE** Removed `app_info` from HDK [1108](https://github.com/holochain/holochain/pull/1108)
- Permissions on host functions now return an error instead of panicking [1141](https://github.com/holochain/holochain/pull/1141)
- Add `--build-info` CLI flag for displaying various information in JSON format. [\#1163](https://github.com/holochain/holochain/pull/1163)

## [holochain\_test\_wasm\_common-0.0.17](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.17)

## [holochain\_cascade-0.0.21](crates/holochain_cascade/CHANGELOG.md#0.0.21)

- Gets won’t return private entries unless you are have committed a header for that entry. [\#1157](https://github.com/holochain/holochain/pull/1157)

## [holochain\_cli-0.0.22](crates/holochain_cli/CHANGELOG.md#0.0.22)

## [holochain\_websocket-0.0.21](crates/holochain_websocket/CHANGELOG.md#0.0.21)

## [holochain\_conductor\_api-0.0.21](crates/holochain_conductor_api/CHANGELOG.md#0.0.21)

## [holochain\_state-0.0.21](crates/holochain_state/CHANGELOG.md#0.0.21)

## [holochain\_wasm\_test\_utils-0.0.21](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.21)

## [holochain\_p2p-0.0.21](crates/holochain_p2p/CHANGELOG.md#0.0.21)

## [holochain\_types-0.0.21](crates/holochain_types/CHANGELOG.md#0.0.21)

## [holochain\_keystore-0.0.21](crates/holochain_keystore/CHANGELOG.md#0.0.21)

## [holochain\_sqlite-0.0.21](crates/holochain_sqlite/CHANGELOG.md#0.0.21)

## [hdk-0.0.117](crates/hdk/CHANGELOG.md#0.0.117)

## [hdk\_derive-0.0.19](crates/hdk_derive/CHANGELOG.md#0.0.19)

## [holochain\_zome\_types-0.0.19](crates/holochain_zome_types/CHANGELOG.md#0.0.19)

## [holo\_hash-0.0.14](crates/holo_hash/CHANGELOG.md#0.0.14)

# 20211215.130843

## [holochain-0.0.120](crates/holochain/CHANGELOG.md#0.0.120)

## [holochain\_cascade-0.0.20](crates/holochain_cascade/CHANGELOG.md#0.0.20)

## [holochain\_cli-0.0.21](crates/holochain_cli/CHANGELOG.md#0.0.21)

## [holochain\_cli\_sandbox-0.0.18](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.18)

## [holochain\_websocket-0.0.20](crates/holochain_websocket/CHANGELOG.md#0.0.20)

## [holochain\_conductor\_api-0.0.20](crates/holochain_conductor_api/CHANGELOG.md#0.0.20)

## [holochain\_state-0.0.20](crates/holochain_state/CHANGELOG.md#0.0.20)

## [holochain\_wasm\_test\_utils-0.0.20](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.20)

## [holochain\_p2p-0.0.20](crates/holochain_p2p/CHANGELOG.md#0.0.20)

## [holochain\_cli\_bundle-0.0.17](crates/holochain_cli_bundle/CHANGELOG.md#0.0.17)

## [holochain\_types-0.0.20](crates/holochain_types/CHANGELOG.md#0.0.20)

## [holochain\_keystore-0.0.20](crates/holochain_keystore/CHANGELOG.md#0.0.20)

## [holochain\_sqlite-0.0.20](crates/holochain_sqlite/CHANGELOG.md#0.0.20)

## [kitsune\_p2p-0.0.18](crates/kitsune_p2p/CHANGELOG.md#0.0.18)

# 20211208.091009

## [holochain-0.0.119](crates/holochain/CHANGELOG.md#0.0.119)

## [holochain\_test\_wasm\_common-0.0.16](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.16)

## [holochain\_cascade-0.0.19](crates/holochain_cascade/CHANGELOG.md#0.0.19)

- Fixes database queries that were running on the runtime thread instead of the background thread. Makes the connections wait for a permit before taking a database connection from the pool. [\#1145](https://github.com/holochain/holochain/pull/1145)

## [holochain\_cli-0.0.20](crates/holochain_cli/CHANGELOG.md#0.0.20)

## [holochain\_cli\_sandbox-0.0.17](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.17)

## [holochain\_websocket-0.0.19](crates/holochain_websocket/CHANGELOG.md#0.0.19)

## [holochain\_conductor\_api-0.0.19](crates/holochain_conductor_api/CHANGELOG.md#0.0.19)

## [holochain\_state-0.0.19](crates/holochain_state/CHANGELOG.md#0.0.19)

## [holochain\_wasm\_test\_utils-0.0.19](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.19)

## [holochain\_p2p-0.0.19](crates/holochain_p2p/CHANGELOG.md#0.0.19)

## [holochain\_cli\_bundle-0.0.16](crates/holochain_cli_bundle/CHANGELOG.md#0.0.16)

## [holochain\_types-0.0.19](crates/holochain_types/CHANGELOG.md#0.0.19)

## [holochain\_keystore-0.0.19](crates/holochain_keystore/CHANGELOG.md#0.0.19)

## [holochain\_sqlite-0.0.19](crates/holochain_sqlite/CHANGELOG.md#0.0.19)

- Adds `basis_hash` index to `DhtOp` table. This makes get queries faster. [\#1143](https://github.com/holochain/holochain/pull/1143)

## [kitsune\_p2p-0.0.17](crates/kitsune_p2p/CHANGELOG.md#0.0.17)

- Agent info is now published as well as gossiped. [\#1115](https://github.com/holochain/holochain/pull/1115)
- BREAKING: Network wire message has changed format so will not be compatible with older versions. [1143](https://github.com/holochain/holochain/pull/1143).
- Fixes to gossip that allows batching of large amounts of data. [1143](https://github.com/holochain/holochain/pull/1143).

## [kitsune\_p2p\_proxy-0.0.15](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.15)

## [kitsune\_p2p\_transport\_quic-0.0.15](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.15)

## [kitsune\_p2p\_types-0.0.15](crates/kitsune_p2p_types/CHANGELOG.md#0.0.15)

## [hdk-0.0.116](crates/hdk/CHANGELOG.md#0.0.116)

## [hdk\_derive-0.0.18](crates/hdk_derive/CHANGELOG.md#0.0.18)

## [holochain\_zome\_types-0.0.18](crates/holochain_zome_types/CHANGELOG.md#0.0.18)

## [holo\_hash-0.0.13](crates/holo_hash/CHANGELOG.md#0.0.13)

## [fixt-0.0.8](crates/fixt/CHANGELOG.md#0.0.8)

# 20211201.111024

## [holochain-0.0.118](crates/holochain/CHANGELOG.md#0.0.118)

- **BREAKING CHANGE** - Gossip now exchanges local peer info with `initiate` and `accept` request types. [\#1114](https://github.com/holochain/holochain/pull/1114).

## [holochain\_cascade-0.0.18](crates/holochain_cascade/CHANGELOG.md#0.0.18)

## [holochain\_cli-0.0.19](crates/holochain_cli/CHANGELOG.md#0.0.19)

## [holochain\_cli\_sandbox-0.0.16](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.16)

## [holochain\_websocket-0.0.18](crates/holochain_websocket/CHANGELOG.md#0.0.18)

## [holochain\_conductor\_api-0.0.18](crates/holochain_conductor_api/CHANGELOG.md#0.0.18)

## [holochain\_state-0.0.18](crates/holochain_state/CHANGELOG.md#0.0.18)

## [holochain\_wasm\_test\_utils-0.0.18](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.18)

## [holochain\_p2p-0.0.18](crates/holochain_p2p/CHANGELOG.md#0.0.18)

## [holochain\_cli\_bundle-0.0.15](crates/holochain_cli_bundle/CHANGELOG.md#0.0.15)

## [holochain\_types-0.0.18](crates/holochain_types/CHANGELOG.md#0.0.18)

## [holochain\_keystore-0.0.18](crates/holochain_keystore/CHANGELOG.md#0.0.18)

## [holochain\_sqlite-0.0.18](crates/holochain_sqlite/CHANGELOG.md#0.0.18)

## [kitsune\_p2p-0.0.16](crates/kitsune_p2p/CHANGELOG.md#0.0.16)

# 20211124.093220

## [holochain-0.0.117](crates/holochain/CHANGELOG.md#0.0.117)

## [holochain\_cascade-0.0.17](crates/holochain_cascade/CHANGELOG.md#0.0.17)

## [holochain\_cli-0.0.18](crates/holochain_cli/CHANGELOG.md#0.0.18)

## [holochain\_cli\_sandbox-0.0.15](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.15)

## [holochain\_websocket-0.0.17](crates/holochain_websocket/CHANGELOG.md#0.0.17)

## [holochain\_conductor\_api-0.0.17](crates/holochain_conductor_api/CHANGELOG.md#0.0.17)

- **BREAKING CHANGES**: db\_sync\_level changes to db\_sync\_strategy. Options are now `Fast` and `Resilient`. Default is `Fast` and should be the standard choice for most use cases. [\#1130](https://github.com/holochain/holochain/pull/1130)

## [holochain\_state-0.0.17](crates/holochain_state/CHANGELOG.md#0.0.17)

- Some databases can handle corruption by wiping the db file and starting again. [\#1039](https://github.com/holochain/holochain/pull/1039).

## [holochain\_wasm\_test\_utils-0.0.17](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.17)

## [holochain\_p2p-0.0.17](crates/holochain_p2p/CHANGELOG.md#0.0.17)

- BREAKING: Wire message `CallRemote` Takes `from_agent`. [\#1091](https://github.com/holochain/holochain/pull/1091)

## [holochain\_cli\_bundle-0.0.14](crates/holochain_cli_bundle/CHANGELOG.md#0.0.14)

## [holochain\_types-0.0.17](crates/holochain_types/CHANGELOG.md#0.0.17)

## [holochain\_keystore-0.0.17](crates/holochain_keystore/CHANGELOG.md#0.0.17)

## [holochain\_sqlite-0.0.17](crates/holochain_sqlite/CHANGELOG.md#0.0.17)

- **BREAKING CHANGES**: All DHT data for the same DNA space is now shared in the same database. All authored data for the same DNA space is also now shared in another database. This requires no changes however data must be manually migrated from the old databases to the new databases. [\#1130](https://github.com/holochain/holochain/pull/1130)

## [kitsune\_p2p-0.0.15](crates/kitsune_p2p/CHANGELOG.md#0.0.15)

- BREAKING: Wire message `Call` no longer takes `from_agent`. [\#1091](https://github.com/holochain/holochain/pull/1091)

## [kitsune\_p2p\_proxy-0.0.14](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.14)

## [kitsune\_p2p\_transport\_quic-0.0.14](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.14)

## [kitsune\_p2p\_types-0.0.14](crates/kitsune_p2p_types/CHANGELOG.md#0.0.14)

## [mr\_bundle-0.0.6](crates/mr_bundle/CHANGELOG.md#0.0.6)

## [holochain\_util-0.0.6](crates/holochain_util/CHANGELOG.md#0.0.6)

# 20211117.094411

## [holochain-0.0.116](crates/holochain/CHANGELOG.md#0.0.116)

## [holochain\_cascade-0.0.16](crates/holochain_cascade/CHANGELOG.md#0.0.16)

## [holochain\_cli-0.0.17](crates/holochain_cli/CHANGELOG.md#0.0.17)

## [holochain\_websocket-0.0.16](crates/holochain_websocket/CHANGELOG.md#0.0.16)

## [holochain\_conductor\_api-0.0.16](crates/holochain_conductor_api/CHANGELOG.md#0.0.16)

## [holochain\_state-0.0.16](crates/holochain_state/CHANGELOG.md#0.0.16)

## [holochain\_wasm\_test\_utils-0.0.16](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.16)

## [holochain\_p2p-0.0.16](crates/holochain_p2p/CHANGELOG.md#0.0.16)

## [holochain\_types-0.0.16](crates/holochain_types/CHANGELOG.md#0.0.16)

## [holochain\_keystore-0.0.16](crates/holochain_keystore/CHANGELOG.md#0.0.16)

## [holochain\_sqlite-0.0.16](crates/holochain_sqlite/CHANGELOG.md#0.0.16)

## [kitsune\_p2p-0.0.14](crates/kitsune_p2p/CHANGELOG.md#0.0.14)

## [kitsune\_p2p\_proxy-0.0.13](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.13)

## [kitsune\_p2p\_transport\_quic-0.0.13](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.13)

## [kitsune\_p2p\_types-0.0.13](crates/kitsune_p2p_types/CHANGELOG.md#0.0.13)

## [mr\_bundle-0.0.5](crates/mr_bundle/CHANGELOG.md#0.0.5)

## [holochain\_util-0.0.5](crates/holochain_util/CHANGELOG.md#0.0.5)

# 20211110.083530

## [holochain-0.0.115](crates/holochain/CHANGELOG.md#0.0.115)

- Fix [issue](https://github.com/holochain/holochain/issues/1100) where private dht ops were being leaked through the incoming ops sender. [1104](https://github.com/holochain/holochain/pull/1104).
- Kitsune now attempts to rebind the network interface in the event of endpoint shutdown. Note, it’s still recommended to bind to `0.0.0.0` as the OS provides additional resiliency for interfaces coming and going. [\#1083](https://github.com/holochain/holochain/pull/1083)
- **BREAKING CHANGE** current chain head including recent writes available in agent info [\#1079](https://github.com/holochain/holochain/pull/1079)
- **BREAKING (If using new lair)** If you are using the new (non-legacy) `lair_server` keystore, you will need to rebuild your keystore, we now pre-hash the passphrase used to access it to mitigate some information leakage. [\#1094](https://github.com/holochain/holochain/pull/1094)
- Better lair signature fallback child process management. The child process will now be properly restarted if it exits. (Note this can take a few millis on Windows, and may result in some signature errors.) [\#1094](https://github.com/holochain/holochain/pull/1094)

## [holochain\_test\_wasm\_common-0.0.15](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.15)

## [holochain\_cascade-0.0.15](crates/holochain_cascade/CHANGELOG.md#0.0.15)

## [holochain\_cli-0.0.16](crates/holochain_cli/CHANGELOG.md#0.0.16)

## [holochain\_cli\_sandbox-0.0.14](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.14)

## [holochain\_websocket-0.0.15](crates/holochain_websocket/CHANGELOG.md#0.0.15)

## [holochain\_conductor\_api-0.0.15](crates/holochain_conductor_api/CHANGELOG.md#0.0.15)

## [holochain\_state-0.0.15](crates/holochain_state/CHANGELOG.md#0.0.15)

## [holochain\_wasm\_test\_utils-0.0.15](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.15)

## [holochain\_p2p-0.0.15](crates/holochain_p2p/CHANGELOG.md#0.0.15)

## [holochain\_cli\_bundle-0.0.13](crates/holochain_cli_bundle/CHANGELOG.md#0.0.13)

## [holochain\_types-0.0.15](crates/holochain_types/CHANGELOG.md#0.0.15)

- FIX: [Bug](https://github.com/holochain/holochain/issues/1101) that was allowing `HeaderWithoutEntry` to shutdown apps. [\#1105](https://github.com/holochain/holochain/pull/1105)

## [holochain\_keystore-0.0.15](crates/holochain_keystore/CHANGELOG.md#0.0.15)

## [holochain\_sqlite-0.0.15](crates/holochain_sqlite/CHANGELOG.md#0.0.15)

- Fixes: Bug where database connections would timeout and return `DatabaseError(DbConnectionPoolError(Error(None)))`. [\#1097](https://github.com/holochain/holochain/pull/1097).

## [kitsune\_p2p-0.0.13](crates/kitsune_p2p/CHANGELOG.md#0.0.13)

## [kitsune\_p2p\_proxy-0.0.12](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.12)

## [kitsune\_p2p\_transport\_quic-0.0.12](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.12)

## [kitsune\_p2p\_types-0.0.12](crates/kitsune_p2p_types/CHANGELOG.md#0.0.12)

## [hdk-0.0.115](crates/hdk/CHANGELOG.md#0.0.115)

## [hdk\_derive-0.0.17](crates/hdk_derive/CHANGELOG.md#0.0.17)

## [holochain\_zome\_types-0.0.17](crates/holochain_zome_types/CHANGELOG.md#0.0.17)

- BREAKING CHANGE: Add all function names in a wasm to the zome info [\#1081](https://github.com/holochain/holochain/pull/1081)
- BREAKING CHANGE: Added a placeholder for zome properties on zome info [\#1080](https://github.com/holochain/holochain/pull/1080)

# 20211103.094627

## [holochain-0.0.114](crates/holochain/CHANGELOG.md#0.0.114)

- `remote_signal` has always been a fire-and-forget operation. Now it also uses the more efficient fire-and-forget “notify” low-level networking plumbing. [\#1075](https://github.com/holochain/holochain/pull/1075)

- **BREAKING CHANGE** `entry_defs` added to `zome_info` and referenced by macros [PR1055](https://github.com/holochain/holochain/pull/1055)

- **BREAKING CHANGE**: The notion of “cell nicknames” (“nicks”) and “app slots” has been unified into the notion of “app roles”. This introduces several breaking changes. In general, you will need to rebuild any app bundles you are using, and potentially update some usages of the admin interface. In particular:
  
  - The `slots` field in App manifests is now called `roles`
  - The `InstallApp` admin method now takes a `role_id` field instead of a `nick` field
  - In the return value for any admin method which lists installed apps, e.g. `ListEnabledApps`, any reference to `"slots"` is now named `"roles"`
  - See [\#1045](https://github.com/holochain/holochain/pull/1045)

- Adds test utils for creating simulated networks. [\#1037](https://github.com/holochain/holochain/pull/1037).

- Conductor can take a mocked network for testing simulated networks. [\#1036](https://github.com/holochain/holochain/pull/1036)

- Added `DumpFullState` to the admin interface, as a more complete form of `DumpState` which returns full `Vec<DhtOp>` instead of just their count, enabling more introspection of the state of the cell [\#1065](https://github.com/holochain/holochain/pull/1065).

- **BREAKING CHANGE** Added function name to call info in HDK. [\#1078](https://github.com/holochain/holochain/pull/1078).

## [holochain\_test\_wasm\_common-0.0.14](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.14)

## [holochain\_cascade-0.0.14](crates/holochain_cascade/CHANGELOG.md#0.0.14)

## [holochain\_cli-0.0.15](crates/holochain_cli/CHANGELOG.md#0.0.15)

## [holochain\_cli\_sandbox-0.0.13](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.13)

## [holochain\_websocket-0.0.14](crates/holochain_websocket/CHANGELOG.md#0.0.14)

## [holochain\_conductor\_api-0.0.14](crates/holochain_conductor_api/CHANGELOG.md#0.0.14)

## [holochain\_state-0.0.14](crates/holochain_state/CHANGELOG.md#0.0.14)

- BREAKING CHANGE. Source chain `query` will now return results in header sequence order ascending.

## [holochain\_wasm\_test\_utils-0.0.14](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.14)

## [holochain\_p2p-0.0.14](crates/holochain_p2p/CHANGELOG.md#0.0.14)

## [holochain\_cli\_bundle-0.0.12](crates/holochain_cli_bundle/CHANGELOG.md#0.0.12)

## [holochain\_types-0.0.14](crates/holochain_types/CHANGELOG.md#0.0.14)

## [holochain\_keystore-0.0.14](crates/holochain_keystore/CHANGELOG.md#0.0.14)

## [holochain\_sqlite-0.0.14](crates/holochain_sqlite/CHANGELOG.md#0.0.14)

## [kitsune\_p2p-0.0.12](crates/kitsune_p2p/CHANGELOG.md#0.0.12)

- BREAKING: Return `ShardedGossipWire::Busy` if we are overloaded with incoming gossip. [\#1076](https://github.com/holochain/holochain/pull/1076)
  - This breaks the current network protocol and will not be compatible with other older versions of holochain (no manual action required).

## [kitsune\_p2p\_proxy-0.0.11](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.11)

## [kitsune\_p2p\_transport\_quic-0.0.11](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.11)

## [kitsune\_p2p\_types-0.0.11](crates/kitsune_p2p_types/CHANGELOG.md#0.0.11)

## [hdk-0.0.114](crates/hdk/CHANGELOG.md#0.0.114)

## [hdk\_derive-0.0.16](crates/hdk_derive/CHANGELOG.md#0.0.16)

## [holochain\_zome\_types-0.0.16](crates/holochain_zome_types/CHANGELOG.md#0.0.16)

## [holo\_hash-0.0.12](crates/holo_hash/CHANGELOG.md#0.0.12)

## [kitsune\_p2p\_dht\_arc-0.0.7](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.7)

# 20211027.100746

## [holochain-0.0.113](crates/holochain/CHANGELOG.md#0.0.113)

- Post commit is now infallible and expects no return value [PR1049](https://github.com/holochain/holochain/pull/1049)
- Always depend on `itertools` to make `cargo build --no-default-features` work [\#1060](https://github.com/holochain/holochain/pull/1060)

## [holochain\_test\_wasm\_common-0.0.13](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.13)

## [holochain\_cascade-0.0.13](crates/holochain_cascade/CHANGELOG.md#0.0.13)

## [holochain\_cli-0.0.14](crates/holochain_cli/CHANGELOG.md#0.0.14)

## [holochain\_cli\_sandbox-0.0.12](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.12)

## [holochain\_websocket-0.0.13](crates/holochain_websocket/CHANGELOG.md#0.0.13)

## [holochain\_conductor\_api-0.0.13](crates/holochain_conductor_api/CHANGELOG.md#0.0.13)

## [holochain\_state-0.0.13](crates/holochain_state/CHANGELOG.md#0.0.13)

## [holochain\_wasm\_test\_utils-0.0.13](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.13)

## [holochain\_p2p-0.0.13](crates/holochain_p2p/CHANGELOG.md#0.0.13)

## [holochain\_cli\_bundle-0.0.11](crates/holochain_cli_bundle/CHANGELOG.md#0.0.11)

## [holochain\_types-0.0.13](crates/holochain_types/CHANGELOG.md#0.0.13)

## [holochain\_keystore-0.0.13](crates/holochain_keystore/CHANGELOG.md#0.0.13)

## [holochain\_sqlite-0.0.13](crates/holochain_sqlite/CHANGELOG.md#0.0.13)

## [hdk-0.0.113](crates/hdk/CHANGELOG.md#0.0.113)

## [hdk\_derive-0.0.15](crates/hdk_derive/CHANGELOG.md#0.0.15)

- `#[hdk_extern(infallible)]` now supports leaving off the return type of a fn [PR1049](https://github.com/holochain/holochain/pull/1049)

## [holochain\_zome\_types-0.0.15](crates/holochain_zome_types/CHANGELOG.md#0.0.15)

- `HeaderHashes` no longer exists [PR1049](https://github.com/holochain/holochain/pull/1049)
- `HeaderHashedVec` no longer exists [PR1049](https://github.com/holochain/holochain/pull/1049)

## [holo\_hash-0.0.11](crates/holo_hash/CHANGELOG.md#0.0.11)

# 20211021.140006

## [holochain-0.0.112](crates/holochain/CHANGELOG.md#0.0.112)

- Always depend on `itertools` to make `cargo build --no-default-features` work [\#1060](https://github.com/holochain/holochain/pull/1060)

## [holochain\_test\_wasm\_common-0.0.12](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.12)

## [holochain\_cascade-0.0.12](crates/holochain_cascade/CHANGELOG.md#0.0.12)

## [holochain\_cli-0.0.13](crates/holochain_cli/CHANGELOG.md#0.0.13)

## [holochain\_cli\_sandbox-0.0.11](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.11)

## [holochain\_websocket-0.0.12](crates/holochain_websocket/CHANGELOG.md#0.0.12)

## [holochain\_conductor\_api-0.0.12](crates/holochain_conductor_api/CHANGELOG.md#0.0.12)

## [holochain\_state-0.0.12](crates/holochain_state/CHANGELOG.md#0.0.12)

## [holochain\_wasm\_test\_utils-0.0.12](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.12)

## [holochain\_p2p-0.0.12](crates/holochain_p2p/CHANGELOG.md#0.0.12)

## [holochain\_cli\_bundle-0.0.10](crates/holochain_cli_bundle/CHANGELOG.md#0.0.10)

## [holochain\_types-0.0.12](crates/holochain_types/CHANGELOG.md#0.0.12)

## [holochain\_keystore-0.0.12](crates/holochain_keystore/CHANGELOG.md#0.0.12)

## [holochain\_sqlite-0.0.12](crates/holochain_sqlite/CHANGELOG.md#0.0.12)

## [hdk-0.0.112](crates/hdk/CHANGELOG.md#0.0.112)

## [hdk\_derive-0.0.14](crates/hdk_derive/CHANGELOG.md#0.0.14)

## [holochain\_zome\_types-0.0.14](crates/holochain_zome_types/CHANGELOG.md#0.0.14)

# 20211020.171211

## [holochain-0.0.111](crates/holochain/CHANGELOG.md#0.0.111)

- `call_info` is now implemented [1047](https://github.com/holochain/holochain/pull/1047)

- `dna_info` now returns `DnaInfo` correctly [\#1044](https://github.com/holochain/holochain/pull/1044)
  
  - `ZomeInfo` no longer includes what is now on `DnaInfo`
  - `ZomeInfo` renames `zome_name` and `zome_id` to `name` and `id`
  - `DnaInfo` includes `name`, `hash`, `properties`

- `post_commit` hook is implemented now [PR 1000](https://github.com/holochain/holochain/pull/1000)

- Bump legacy lair version to 0.0.8 fixing a crash when error message was too long [\#1046](https://github.com/holochain/holochain/pull/1046)

- Options to use new lair keystore [\#1040](https://github.com/holochain/holochain/pull/1040)

<!-- end list -->

``` yaml
keystore:
  type: danger_test_keystore
```

or

``` yaml
keystore:
  type: lair_server
  connection_url: "unix:///my/path/socket?k=Foo"
```

## [holochain\_test\_wasm\_common-0.0.11](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.11)

## [holochain\_cascade-0.0.11](crates/holochain_cascade/CHANGELOG.md#0.0.11)

## [holochain\_cli-0.0.12](crates/holochain_cli/CHANGELOG.md#0.0.12)

## [holochain\_cli\_sandbox-0.0.10](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.10)

## [holochain\_websocket-0.0.11](crates/holochain_websocket/CHANGELOG.md#0.0.11)

## [holochain\_conductor\_api-0.0.11](crates/holochain_conductor_api/CHANGELOG.md#0.0.11)

## [holochain\_state-0.0.11](crates/holochain_state/CHANGELOG.md#0.0.11)

## [holochain\_wasm\_test\_utils-0.0.11](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.11)

## [holochain\_p2p-0.0.11](crates/holochain_p2p/CHANGELOG.md#0.0.11)

## [holochain\_cli\_bundle-0.0.9](crates/holochain_cli_bundle/CHANGELOG.md#0.0.9)

## [holochain\_types-0.0.11](crates/holochain_types/CHANGELOG.md#0.0.11)

## [holochain\_keystore-0.0.11](crates/holochain_keystore/CHANGELOG.md#0.0.11)

## [holochain\_sqlite-0.0.11](crates/holochain_sqlite/CHANGELOG.md#0.0.11)

## [kitsune\_p2p-0.0.11](crates/kitsune_p2p/CHANGELOG.md#0.0.11)

## [kitsune\_p2p\_proxy-0.0.10](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.10)

## [kitsune\_p2p\_transport\_quic-0.0.10](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.10)

## [kitsune\_p2p\_types-0.0.10](crates/kitsune_p2p_types/CHANGELOG.md#0.0.10)

## [hdk-0.0.111](crates/hdk/CHANGELOG.md#0.0.111)

## [hdk\_derive-0.0.13](crates/hdk_derive/CHANGELOG.md#0.0.13)

## [holochain\_zome\_types-0.0.13](crates/holochain_zome_types/CHANGELOG.md#0.0.13)

- `CallInfo` now has `as_at` on it [PR 1047](https://github.com/holochain/holochain/pull/1047)
- Removed `Links` in favour of `Vec<Link>` [PR 1012](https://github.com/holochain/holochain/pull/1012)

## [holo\_hash-0.0.10](crates/holo_hash/CHANGELOG.md#0.0.10)

# 20211013.091723

## [holochain-0.0.110](crates/holochain/CHANGELOG.md#0.0.110)

- Publish now runs on a loop if there are ops still needing receipts. [\#1024](https://github.com/holochain/holochain/pull/1024)
- Batch peer store write so we use less transactions. [\#1007](https://github.com/holochain/holochain/pull/1007/).
- Preparation for new lair api [\#1017](https://github.com/holochain/holochain/pull/1017)
  - there should be no functional changes with this update.
  - adds new lair as an additional dependency and begins preparation for a config-time switch allowing use of new api lair keystore.
- Add method `SweetDnaFile::from_bundle_with_overrides` [\#1030](https://github.com/holochain/holochain/pull/1030)
- Some `SweetConductor::setup_app_*` methods now take anything iterable, instead of array slices, for specifying lists of agents and DNAs [\#1030](https://github.com/holochain/holochain/pull/1030)
- BREAKING conductor config changes [\#1031](https://github.com/holochain/holochain/pull/1031)

Where previously, you might have had:

``` yaml
use_dangerous_test_keystore: false
keystore_path: /my/path
passphrase_service:
  type: danger_insecure_from_config
  passphrase: "test-passphrase"
```

now you will use:

``` yaml
keystore:
  type: lair_server_legacy_deprecated
  keystore_path: /my/path
  danger_passphrase_insecure_from_config: "test-passphrase"
```

or:

``` yaml
keystore:
  type: danger_test_keystore_legacy_deprecated
```

## [holochain\_test\_wasm\_common-0.0.10](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.10)

## [holochain\_cascade-0.0.10](crates/holochain_cascade/CHANGELOG.md#0.0.10)

- Fix authority side get\_links query [\#1027](https://github.com/holochain/holochain/pull/1027).

## [holochain\_cli-0.0.11](crates/holochain_cli/CHANGELOG.md#0.0.11)

## [holochain\_cli\_sandbox-0.0.9](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.9)

## [holochain\_websocket-0.0.10](crates/holochain_websocket/CHANGELOG.md#0.0.10)

## [holochain\_conductor\_api-0.0.10](crates/holochain_conductor_api/CHANGELOG.md#0.0.10)

## [holochain\_state-0.0.10](crates/holochain_state/CHANGELOG.md#0.0.10)

## [holochain\_wasm\_test\_utils-0.0.10](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.10)

## [holochain\_p2p-0.0.10](crates/holochain_p2p/CHANGELOG.md#0.0.10)

## [holochain\_cli\_bundle-0.0.8](crates/holochain_cli_bundle/CHANGELOG.md#0.0.8)

## [holochain\_types-0.0.10](crates/holochain_types/CHANGELOG.md#0.0.10)

## [holochain\_keystore-0.0.10](crates/holochain_keystore/CHANGELOG.md#0.0.10)

## [holochain\_sqlite-0.0.10](crates/holochain_sqlite/CHANGELOG.md#0.0.10)

## [kitsune\_p2p-0.0.10](crates/kitsune_p2p/CHANGELOG.md#0.0.10)

- Check local agents for basis when doing a RPCMulti call. [\#1009](https://github.com/holochain/holochain/pull/1009).

## [kitsune\_p2p\_proxy-0.0.9](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.9)

## [kitsune\_p2p\_transport\_quic-0.0.9](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.9)

## [kitsune\_p2p\_types-0.0.9](crates/kitsune_p2p_types/CHANGELOG.md#0.0.9)

## [hdk-0.0.110](crates/hdk/CHANGELOG.md#0.0.110)

## [hdk\_derive-0.0.12](crates/hdk_derive/CHANGELOG.md#0.0.12)

## [holochain\_zome\_types-0.0.12](crates/holochain_zome_types/CHANGELOG.md#0.0.12)

## [holo\_hash-0.0.9](crates/holo_hash/CHANGELOG.md#0.0.9)

## [kitsune\_p2p\_dht\_arc-0.0.6](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.6)

## [fixt-0.0.7](crates/fixt/CHANGELOG.md#0.0.7)

# 20211006.105406

## [holochain-0.0.109](crates/holochain/CHANGELOG.md#0.0.109)

- Make validation run concurrently up to 50 DhtOps. This allows us to make progress on other ops when waiting for the network. [\#1005](https://github.com/holochain/holochain/pull/1005)
- FIX: Prevent the conductor from trying to join cells to the network that are already in the process of joining. [\#1006](https://github.com/holochain/holochain/pull/1006)

## [holochain\_test\_wasm\_common-0.0.9](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.9)

## [holochain\_cascade-0.0.9](crates/holochain_cascade/CHANGELOG.md#0.0.9)

## [holochain\_cli-0.0.10](crates/holochain_cli/CHANGELOG.md#0.0.10)

## [holochain\_cli\_sandbox-0.0.8](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.8)

## [holochain\_websocket-0.0.9](crates/holochain_websocket/CHANGELOG.md#0.0.9)

## [holochain\_conductor\_api-0.0.9](crates/holochain_conductor_api/CHANGELOG.md#0.0.9)

## [holochain\_state-0.0.9](crates/holochain_state/CHANGELOG.md#0.0.9)

- Fixed a bug when creating an entry with `ChainTopOrdering::Relaxed`, in which the header was created and stored in the Source Chain, but the actual entry was not.
- Geneis ops will no longer run validation for the authored node and only genesis self check will run. [\#995](https://github.com/holochain/holochain/pull/995)

## [holochain\_wasm\_test\_utils-0.0.9](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.9)

## [holochain\_p2p-0.0.9](crates/holochain_p2p/CHANGELOG.md#0.0.9)

## [holochain\_cli\_bundle-0.0.7](crates/holochain_cli_bundle/CHANGELOG.md#0.0.7)

## [holochain\_types-0.0.9](crates/holochain_types/CHANGELOG.md#0.0.9)

## [holochain\_keystore-0.0.9](crates/holochain_keystore/CHANGELOG.md#0.0.9)

- Update to lair 0.0.7 which updates to rusqlite 0.26.0 [\#1023](https://github.com/holochain/holochain/pull/1023)
  - provides `bundled-sqlcipher-vendored-openssl` to ease build process on non-windows systems (windows is still using `bundled` which doesn’t provide at-rest encryption).

## [holochain\_sqlite-0.0.9](crates/holochain_sqlite/CHANGELOG.md#0.0.9)

- Update to rusqlite 0.26.0 [\#1023](https://github.com/holochain/holochain/pull/1023)
  - provides `bundled-sqlcipher-vendored-openssl` to ease build process on non-windows systems (windows is still using `bundled` which doesn’t provide at-rest encryption).

## [kitsune\_p2p-0.0.9](crates/kitsune_p2p/CHANGELOG.md#0.0.9)

- Fix rpc\_multi bug that caused all request to wait 3 seconds. [\#1009](https://github.com/holochain/holochain/pull/1009/)
- Fix to gossip’s round initiate. We were not timing out a round if there was no response to an initiate message. [\#1014](https://github.com/holochain/holochain/pull/1014).
- Make gossip only initiate with agents that have info that is not expired. [\#1014](https://github.com/holochain/holochain/pull/1014).

## [kitsune\_p2p\_proxy-0.0.8](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.8)

## [kitsune\_p2p\_transport\_quic-0.0.8](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.8)

## [kitsune\_p2p\_types-0.0.8](crates/kitsune_p2p_types/CHANGELOG.md#0.0.8)

## [kitsune\_p2p\_dht\_arc-0.0.5](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.5)

## [hdk-0.0.109](crates/hdk/CHANGELOG.md#0.0.109)

## [hdk\_derive-0.0.11](crates/hdk_derive/CHANGELOG.md#0.0.11)

## [holochain\_zome\_types-0.0.11](crates/holochain_zome_types/CHANGELOG.md#0.0.11)

## [kitsune\_p2p\_timestamp-0.0.5](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.5)

## [holo\_hash-0.0.8](crates/holo_hash/CHANGELOG.md#0.0.8)

# 20210929.090317

## [holochain-0.0.108](crates/holochain/CHANGELOG.md#0.0.108)

- Refactor conductor to use parking lot rw lock instead of tokio rw lock. (Faster and prevents deadlocks.). [\#979](https://github.com/holochain/holochain/pull/979).

### Changed

- The scheduler should work now

## [holochain\_test\_wasm\_common-0.0.8](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.8)

## [holochain\_cascade-0.0.8](crates/holochain_cascade/CHANGELOG.md#0.0.8)

## [holochain\_cli-0.0.9](crates/holochain_cli/CHANGELOG.md#0.0.9)

## [holochain\_websocket-0.0.8](crates/holochain_websocket/CHANGELOG.md#0.0.8)

## [holochain\_conductor\_api-0.0.8](crates/holochain_conductor_api/CHANGELOG.md#0.0.8)

## [holochain\_state-0.0.8](crates/holochain_state/CHANGELOG.md#0.0.8)

## [holochain\_wasm\_test\_utils-0.0.8](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.8)

## [holochain\_p2p-0.0.8](crates/holochain_p2p/CHANGELOG.md#0.0.8)

## [holochain\_types-0.0.8](crates/holochain_types/CHANGELOG.md#0.0.8)

## [holochain\_keystore-0.0.8](crates/holochain_keystore/CHANGELOG.md#0.0.8)

## [holochain\_sqlite-0.0.8](crates/holochain_sqlite/CHANGELOG.md#0.0.8)

## [kitsune\_p2p-0.0.8](crates/kitsune_p2p/CHANGELOG.md#0.0.8)

### Changed

- `query_gossip_agents`, `query_agent_info_signed`, and `query_agent_info_signed_near_basis` are now unified into a single `query_agents` call in `KitsuneP2pEvent`

## [kitsune\_p2p\_proxy-0.0.7](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.7)

## [kitsune\_p2p\_transport\_quic-0.0.7](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.7)

## [kitsune\_p2p\_types-0.0.7](crates/kitsune_p2p_types/CHANGELOG.md#0.0.7)

- Adds a prototype protocol for checking consistency in a sharded network.

## [kitsune\_p2p\_dht\_arc-0.0.4](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.4)

## [hdk-0.0.108](crates/hdk/CHANGELOG.md#0.0.108)

## [hdk\_derive-0.0.10](crates/hdk_derive/CHANGELOG.md#0.0.10)

### Added

- Added support for `#[hdk_extern(infallible)]`

## [holochain\_zome\_types-0.0.10](crates/holochain_zome_types/CHANGELOG.md#0.0.10)

## [kitsune\_p2p\_timestamp-0.0.4](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.4)

# 20210922.083906

- Adds experimental feature for one storage agent per space to kitsune tuning params. `gossip_single_storage_arc_per_space`.
- Adds the ability to lower the synchronous level for the sqlite backend to the conductor config. `db_sync_level`. See [sqlite documentation](https://www.sqlite.org/pragma.html#pragma_synchronous). This allows running on slower HDD but can result in corrupted databases and is not recommended for production or SSDs.
- Fixes bug where WAL mode was set on every opening connection.

## [holochain-0.0.107](crates/holochain/CHANGELOG.md#0.0.107)

## [holochain\_test\_wasm\_common-0.0.7](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.7)

## [holochain\_cascade-0.0.7](crates/holochain_cascade/CHANGELOG.md#0.0.7)

## [holochain\_cli-0.0.8](crates/holochain_cli/CHANGELOG.md#0.0.8)

## [holochain\_cli\_sandbox-0.0.7](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.7)

## [holochain\_websocket-0.0.7](crates/holochain_websocket/CHANGELOG.md#0.0.7)

## [holochain\_conductor\_api-0.0.7](crates/holochain_conductor_api/CHANGELOG.md#0.0.7)

## [holochain\_state-0.0.7](crates/holochain_state/CHANGELOG.md#0.0.7)

## [holochain\_wasm\_test\_utils-0.0.7](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.7)

## [holochain\_p2p-0.0.7](crates/holochain_p2p/CHANGELOG.md#0.0.7)

## [holochain\_cli\_bundle-0.0.6](crates/holochain_cli_bundle/CHANGELOG.md#0.0.6)

## [holochain\_types-0.0.7](crates/holochain_types/CHANGELOG.md#0.0.7)

- Added helper functions to `WebAppBundle` and `AppManifest` to be able to handle these types better in consuming applications.

## [holochain\_keystore-0.0.7](crates/holochain_keystore/CHANGELOG.md#0.0.7)

## [holochain\_sqlite-0.0.7](crates/holochain_sqlite/CHANGELOG.md#0.0.7)

## [kitsune\_p2p-0.0.7](crates/kitsune_p2p/CHANGELOG.md#0.0.7)

## [kitsune\_p2p\_proxy-0.0.6](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.6)

## [kitsune\_p2p\_transport\_quic-0.0.6](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.6)

## [kitsune\_p2p\_types-0.0.6](crates/kitsune_p2p_types/CHANGELOG.md#0.0.6)

## [kitsune\_p2p\_dht\_arc-0.0.3](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.3)

## [kitsune\_p2p\_mdns-0.0.2](crates/kitsune_p2p_mdns/CHANGELOG.md#0.0.2)

## [mr\_bundle-0.0.4](crates/mr_bundle/CHANGELOG.md#0.0.4)

## [holochain\_util-0.0.4](crates/holochain_util/CHANGELOG.md#0.0.4)

## [hdk-0.0.107](crates/hdk/CHANGELOG.md#0.0.107)

### Changed

- hdk: `schedule` function now takes a String giving a function name to schedule, rather than a Duration

## [hdk\_derive-0.0.9](crates/hdk_derive/CHANGELOG.md#0.0.9)

## [holochain\_zome\_types-0.0.9](crates/holochain_zome_types/CHANGELOG.md#0.0.9)

### Added

- Added `Schedule` enum to define schedules

## [kitsune\_p2p\_timestamp-0.0.3](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.3)

## [holo\_hash-0.0.7](crates/holo_hash/CHANGELOG.md#0.0.7)

## [fixt-0.0.6](crates/fixt/CHANGELOG.md#0.0.6)

# 20210916.085414

## [holochain-0.0.106](crates/holochain/CHANGELOG.md#0.0.106)

### Changed

- HDK `sys_time` now returns a `holochain_zome_types::Timestamp` instead of a `core::time::Duration`.
- Exposes `UninstallApp` in the conductor admin API.

## [holochain\_test\_wasm\_common-0.0.6](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.6)

## [holochain\_cascade-0.0.6](crates/holochain_cascade/CHANGELOG.md#0.0.6)

## [holochain\_cli-0.0.7](crates/holochain_cli/CHANGELOG.md#0.0.7)

- Added the `hc web-app` sub-command for bundling up a UI with a previously created hApp bundle.  It uses the same same behavior as `hc dna` and `hc app` to specify the .yaml manifest file.

## [holochain\_cli\_sandbox-0.0.6](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.6)

- Added `UninstallApp` command.

## [holochain\_websocket-0.0.6](crates/holochain_websocket/CHANGELOG.md#0.0.6)

## [holochain\_conductor\_api-0.0.6](crates/holochain_conductor_api/CHANGELOG.md#0.0.6)

## [holochain\_state-0.0.6](crates/holochain_state/CHANGELOG.md#0.0.6)

## [holochain\_wasm\_test\_utils-0.0.6](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.6)

## [holochain\_p2p-0.0.6](crates/holochain_p2p/CHANGELOG.md#0.0.6)

## [holochain\_cli\_bundle-0.0.5](crates/holochain_cli_bundle/CHANGELOG.md#0.0.5)

- Added the `hc web-app` subcommand, with the exact same behaviour and functionality as `hc dna` and `hc app`.

## [holochain\_types-0.0.6](crates/holochain_types/CHANGELOG.md#0.0.6)

- Added `WebAppManifest` to support `.webhapp` bundles. This is necessary to package hApps together with web UIs, to export to the Launcher and Holo.

## [holochain\_keystore-0.0.6](crates/holochain_keystore/CHANGELOG.md#0.0.6)

## [holochain\_sqlite-0.0.6](crates/holochain_sqlite/CHANGELOG.md#0.0.6)

## [kitsune\_p2p-0.0.6](crates/kitsune_p2p/CHANGELOG.md#0.0.6)

## [kitsune\_p2p\_proxy-0.0.5](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.5)

## [kitsune\_p2p\_transport\_quic-0.0.5](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.5)

## [kitsune\_p2p\_types-0.0.5](crates/kitsune_p2p_types/CHANGELOG.md#0.0.5)

## [hdk-0.0.106](crates/hdk/CHANGELOG.md#0.0.106)

## [hdk\_derive-0.0.8](crates/hdk_derive/CHANGELOG.md#0.0.8)

## [holochain\_zome\_types-0.0.8](crates/holochain_zome_types/CHANGELOG.md#0.0.8)

## [kitsune\_p2p\_timestamp-0.0.2](crates/kitsune_p2p_timestamp/CHANGELOG.md#0.0.2)

## [holo\_hash-0.0.6](crates/holo_hash/CHANGELOG.md#0.0.6)

### Fixed

- Crate now builds with `--no-default-features`

# 20210901.105419

***Note***: The following crates could not be published to crates.io due to build errors:

- hdk\_derive-0.0.7
- hdk-0.0.105
- holochain\_state-0.0.5
- holochain\_conductor\_api-0.0.5
- holochain\_cascade-0.0.5”,
- holochain\_test\_wasm\_common-0.0.5
- holochain-0.0.105

## [holochain-0.0.105](crates/holochain/CHANGELOG.md#0.0.105)

## [holochain\_test\_wasm\_common-0.0.5](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.5)

## [holochain\_cascade-0.0.5](crates/holochain_cascade/CHANGELOG.md#0.0.5)

## [holochain\_cli-0.0.6](crates/holochain_cli/CHANGELOG.md#0.0.6)

## [holochain\_websocket-0.0.5](crates/holochain_websocket/CHANGELOG.md#0.0.5)

## [holochain\_conductor\_api-0.0.5](crates/holochain_conductor_api/CHANGELOG.md#0.0.5)

## [holochain\_state-0.0.5](crates/holochain_state/CHANGELOG.md#0.0.5)

## [holochain\_wasm\_test\_utils-0.0.5](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.5)

## [holochain\_p2p-0.0.5](crates/holochain_p2p/CHANGELOG.md#0.0.5)

## [holochain\_types-0.0.5](crates/holochain_types/CHANGELOG.md#0.0.5)

## [holochain\_keystore-0.0.5](crates/holochain_keystore/CHANGELOG.md#0.0.5)

## [holochain\_sqlite-0.0.5](crates/holochain_sqlite/CHANGELOG.md#0.0.5)

## [kitsune\_p2p-0.0.5](crates/kitsune_p2p/CHANGELOG.md#0.0.5)

## [hdk-0.0.105](crates/hdk/CHANGELOG.md#0.0.105)

## [hdk\_derive-0.0.7](crates/hdk_derive/CHANGELOG.md#0.0.7)

## [holochain\_zome\_types-0.0.7](crates/holochain_zome_types/CHANGELOG.md#0.0.7)

# 20210825.101130

## [holochain-0.0.104](crates/holochain/CHANGELOG.md#0.0.104)

- Updates lair to 0.0.4 which pins rcgen to 0.8.11 to work around [https://github.com/est31/rcgen/issues/63](https://github.com/est31/rcgen/issues/63)

## [holochain\_test\_wasm\_common-0.0.4](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.4)

## [holochain\_cascade-0.0.4](crates/holochain_cascade/CHANGELOG.md#0.0.4)

## [holochain\_cli-0.0.5](crates/holochain_cli/CHANGELOG.md#0.0.5)

## [holochain\_cli\_sandbox-0.0.5](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.5)

## [holochain\_websocket-0.0.4](crates/holochain_websocket/CHANGELOG.md#0.0.4)

## [holochain\_conductor\_api-0.0.4](crates/holochain_conductor_api/CHANGELOG.md#0.0.4)

## [holochain\_state-0.0.4](crates/holochain_state/CHANGELOG.md#0.0.4)

## [holochain\_wasm\_test\_utils-0.0.4](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.4)

## [holochain\_p2p-0.0.4](crates/holochain_p2p/CHANGELOG.md#0.0.4)

## [holochain\_cli\_bundle-0.0.4](crates/holochain_cli_bundle/CHANGELOG.md#0.0.4)

## [holochain\_types-0.0.4](crates/holochain_types/CHANGELOG.md#0.0.4)

## [holochain\_keystore-0.0.4](crates/holochain_keystore/CHANGELOG.md#0.0.4)

## [holochain\_sqlite-0.0.4](crates/holochain_sqlite/CHANGELOG.md#0.0.4)

## [kitsune\_p2p-0.0.4](crates/kitsune_p2p/CHANGELOG.md#0.0.4)

## [kitsune\_p2p\_proxy-0.0.4](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.4)

## [kitsune\_p2p\_transport\_quic-0.0.4](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.4)

## [kitsune\_p2p\_types-0.0.4](crates/kitsune_p2p_types/CHANGELOG.md#0.0.4)

## [hdk-0.0.104](crates/hdk/CHANGELOG.md#0.0.104)

## [hdk\_derive-0.0.6](crates/hdk_derive/CHANGELOG.md#0.0.6)

## [holochain\_zome\_types-0.0.6](crates/holochain_zome_types/CHANGELOG.md#0.0.6)

### Changed

- `CreateInput`, `DeleteInput`, `DeleteLinkInput` structs invented for zome io
- `EntryDefId` merged into `CreateInput`

### Added

- `ChainTopOrdering` enum added to define chain top ordering behaviour on write

# 20210817.185301

## [holochain-0.0.103](crates/holochain/CHANGELOG.md#0.0.103)

### Fixed

- This release solves the issues with installing happ bundles or registering DNA via the admin API concurrently. [\#881](https://github.com/holochain/holochain/pull/881).

### Changed

- Header builder now uses chain top timestamp for new headers if in the future
- Timestamps in headers require strict inequality in sys validation

## [holochain\_test\_wasm\_common-0.0.3](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.3)

## [holochain\_cascade-0.0.3](crates/holochain_cascade/CHANGELOG.md#0.0.3)

## [holochain\_cli-0.0.4](crates/holochain_cli/CHANGELOG.md#0.0.4)

## [holochain\_cli\_sandbox-0.0.4](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.4)

## [holochain\_websocket-0.0.3](crates/holochain_websocket/CHANGELOG.md#0.0.3)

## [holochain\_conductor\_api-0.0.3](crates/holochain_conductor_api/CHANGELOG.md#0.0.3)

- BREAKING: CONDUCTOR CONFIG CHANGE–related to update to lair 0.0.3
  - `passphrase_service` is now required
    - The only implemented option is `danger_insecure_from_config`

#### Example

``` yaml
---
passphrase_service:
  type: danger_insecure_from_config
  passphrase: "foobar"
```

## [holochain\_state-0.0.3](crates/holochain_state/CHANGELOG.md#0.0.3)

## [holochain\_wasm\_test\_utils-0.0.3](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.3)

## [holochain\_p2p-0.0.3](crates/holochain_p2p/CHANGELOG.md#0.0.3)

## [holochain\_cli\_bundle-0.0.3](crates/holochain_cli_bundle/CHANGELOG.md#0.0.3)

## [holochain\_types-0.0.3](crates/holochain_types/CHANGELOG.md#0.0.3)

## [holochain\_keystore-0.0.3](crates/holochain_keystore/CHANGELOG.md#0.0.3)

- Updated to lair 0.0.3
  - switch to sqlite/sqlcipher for keystore backing database
  - enable encryption via passphrase (not on windows)

## [holochain\_sqlite-0.0.3](crates/holochain_sqlite/CHANGELOG.md#0.0.3)

## [kitsune\_p2p-0.0.3](crates/kitsune_p2p/CHANGELOG.md#0.0.3)

## [kitsune\_p2p\_proxy-0.0.3](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.3)

## [kitsune\_p2p\_transport\_quic-0.0.3](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.3)

## [kitsune\_p2p\_types-0.0.3](crates/kitsune_p2p_types/CHANGELOG.md#0.0.3)

## [kitsune\_p2p\_dht\_arc-0.0.2](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.2)

## [mr\_bundle-0.0.3](crates/mr_bundle/CHANGELOG.md#0.0.3)

## [holochain\_util-0.0.3](crates/holochain_util/CHANGELOG.md#0.0.3)

## [hdk-0.0.103](crates/hdk/CHANGELOG.md#0.0.103)

### Changed

- hdk: `sys_time` returns `Timestamp` instead of `Duration`

### Added

- hdk: Added `accept_countersigning_preflight_request`

- hdk: Added `session_times_from_millis`

- hdk: Now supports creating and updating countersigned entries

- hdk: Now supports deserializing countersigned entries in app entry `try_from`

- hdk: implements multi-call for:
  
  - `remote_call`
  - `call`
  - `get`
  - `get_details`
  - `get_links`
  - `get_link_details`
  
  We strictly only needed `remote_call` for countersigning, but feedback from the community was that having to sequentially loop over these common HDK functions is a pain point, so we enabled all of them to be async over a vector of inputs.

## [hdk\_derive-0.0.5](crates/hdk_derive/CHANGELOG.md#0.0.5)

## [holochain\_zome\_types-0.0.5](crates/holochain_zome_types/CHANGELOG.md#0.0.5)

### Added

- Countersigning related functions and structs

## [holo\_hash-0.0.5](crates/holo_hash/CHANGELOG.md#0.0.5)

## [fixt-0.0.5](crates/fixt/CHANGELOG.md#0.0.5)

# 20210722.172107

## [holochain-0.0.102](crates/holochain/CHANGELOG.md#0.0.102)

### Known Issues :exclamation:

- We’ve become aware of a bug that locks up the conductor when installing happ bundles or registering DNA via the admin API concurrently. Please perform these actions sequentially until we’ve resolved the bug.

### Fixed

- Concurrent zome calls could cause the `init()` zome callback to run multiple times concurrently, causing `HeadMoved` errors. This is fixed, so that `init()` can only ever run once.
  - If a zome call has been waiting for another zome call to finish running `init()` for longer than 30 seconds, it will timeout.

### Changed

- Apps now have a more complex status. Apps now can be either enabled/disabled as well as running/stopped, the combination of which is captured by three distinctly named states:
  - “Running” (enabled + running) -\> The app is running normally
  - “Paused” (enabled + stopped) -\> The app is currently stopped due to some minor problem in one of its cells such as failed network access, but will start running again as soon as it’s able. Some Cells may still be running normally.
  - “Disabled” (disabled + stopped) -\> The app is stopped and will remain so until explicitly enabled via `EnableApp` admin method. Apps can be disabled manually via `DisableApp`, or automatically due to an unrecoverable error in a Cell.
- Some admin methods are deprecated due to the app status changes:
  - `ActivateApp` is deprecated in favor of `EnableApp`
  - `DeactivateApp` is deprecated in favor of `DisableApp`
- Apps will be automatically Paused if not all of their cells are able to join the network during startup

### Added

- `InstallAppBundle` command added to admin conductor API. [\#665](https://github.com/holochain/holochain/pull/665)
- `DnaSource` in conductor\_api `RegisterDna` call now can take a `DnaBundle` [\#665](https://github.com/holochain/holochain/pull/665)
- New admin interface methods:
  - `EnableApp` (replaces `ActivateApp`)
  - `DisableApp` (replaces `DeactivateApp`)
  - `StartApp` (used to attempt to manually restart a Paused app)
- Using the 3 level PLRU instance cache from latest holochain wasmer `v0.0.72`

## [holochain\_test\_wasm\_common-0.0.2](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.2)

## [holochain\_cascade-0.0.2](crates/holochain_cascade/CHANGELOG.md#0.0.2)

## [holochain\_cli-0.0.3](crates/holochain_cli/CHANGELOG.md#0.0.3)

## [holochain\_cli\_sandbox-0.0.3](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.3)

## [holochain\_websocket-0.0.2](crates/holochain_websocket/CHANGELOG.md#0.0.2)

## [holochain\_conductor\_api-0.0.2](crates/holochain_conductor_api/CHANGELOG.md#0.0.2)

## [holochain\_state-0.0.2](crates/holochain_state/CHANGELOG.md#0.0.2)

## [holochain\_wasm\_test\_utils-0.0.2](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.2)

## [holochain\_p2p-0.0.2](crates/holochain_p2p/CHANGELOG.md#0.0.2)

## [holochain\_cli\_bundle-0.0.2](crates/holochain_cli_bundle/CHANGELOG.md#0.0.2)

## [holochain\_types-0.0.2](crates/holochain_types/CHANGELOG.md#0.0.2)

## [holochain\_keystore-0.0.2](crates/holochain_keystore/CHANGELOG.md#0.0.2)

## [holochain\_sqlite-0.0.2](crates/holochain_sqlite/CHANGELOG.md#0.0.2)

## [kitsune\_p2p-0.0.2](crates/kitsune_p2p/CHANGELOG.md#0.0.2)

## [kitsune\_p2p\_proxy-0.0.2](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.2)

## [kitsune\_p2p\_transport\_quic-0.0.2](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.2)

## [kitsune\_p2p\_types-0.0.2](crates/kitsune_p2p_types/CHANGELOG.md#0.0.2)

## [mr\_bundle-0.0.2](crates/mr_bundle/CHANGELOG.md#0.0.2)

## [holochain\_util-0.0.2](crates/holochain_util/CHANGELOG.md#0.0.2)

## [hdk-0.0.102](crates/hdk/CHANGELOG.md#0.0.102)

### Changed

- hdk: fixed wrong order of recipient and sender in `x_25519_x_salsa20_poly1305_decrypt`

## [hdk\_derive-0.0.4](crates/hdk_derive/CHANGELOG.md#0.0.4)

## [holochain\_zome\_types-0.0.4](crates/holochain_zome_types/CHANGELOG.md#0.0.4)

## [holo\_hash-0.0.4](crates/holo_hash/CHANGELOG.md#0.0.4)

## [fixt-0.0.4](crates/fixt/CHANGELOG.md#0.0.4)

# 20210624.155736

***:exclamation: Performance impact***

Please navigate to the holochain crate release notes further down for details on the performance impact in this release.

## [holochain-0.0.101](crates/holochain/CHANGELOG.md#0.0.101)

This version contains breaking changes to the conductor API as well as a major upgrade to the underlying Wasm runtime.

***:exclamation: Performance impact***

The version of wasmer that is used in this holochain release contains bugs in the scoping of wasmer modules vs. instances, such that it blocks the proper release of memory and slows down execution of concurrent Wasm instances. While we were able to at least mitigate these effects and are coordinating with wasmer to find a proper solution as soon as possible.

The severity of these issues increases with cell concurrency, i.e. using multiple cells with the same DNA. Application development with a single conductor and a few cells are expected to work well unless your machine has serious resource restrictions.

### Added

- `InstallAppBundle` command added to admin conductor API. [\#665](https://github.com/holochain/holochain/pull/665)
- `DnaSource` in conductor\_api `RegisterDna` call now can take a `DnaBundle` [\#665](https://github.com/holochain/holochain/pull/665)

### Removed

- BREAKING:  `InstallAppDnaPayload` in admin conductor API `InstallApp` command now only accepts a hash.  Both properties and path have been removed as per deprecation warning.  Use either `RegisterDna` or `InstallAppBundle` instead. [\#665](https://github.com/holochain/holochain/pull/665)
- BREAKING: `DnaSource(Path)` in conductor\_api `RegisterDna` call now must point to `DnaBundle` as created by `hc dna pack` not a `DnaFile` created by `dna_util` [\#665](https://github.com/holochain/holochain/pull/665)

### CHANGED

- Updated to a version of `holochain_wasmer` that includes a migration to wasmer v2+. [\#773](https://github.com/holochain/holochain/pull/773/files), [\#801](https://github.com/holochain/holochain/pull/80), [\#836](https://github.com/holochain/holochain/pull/836)
- Introduced a simple instance cache to mitigate and potentially outweigh the effects of the aforementioned wasmer conditions [\#848](https://github.com/holochain/holochain/pull/848)

## [holochain\_test\_wasm\_common-0.0.1](crates/holochain_test_wasm_common/CHANGELOG.md#0.0.1)

## [holochain\_cascade-0.0.1](crates/holochain_cascade/CHANGELOG.md#0.0.1)

## [holochain\_cli-0.0.2](crates/holochain_cli/CHANGELOG.md#0.0.2)

### Removed

- temporarily removed `install_app` from `hc`: its not clear if we should restore yet as mostly should be using `install_app_bundle` [\#665](https://github.com/holochain/holochain/pull/665)

## [holochain\_cli\_sandbox-0.0.2](crates/holochain_cli_sandbox/CHANGELOG.md#0.0.2)

## [holochain\_websocket-0.0.1](crates/holochain_websocket/CHANGELOG.md#0.0.1)

## [holochain\_conductor\_api-0.0.1](crates/holochain_conductor_api/CHANGELOG.md#0.0.1)

## [holochain\_state-0.0.1](crates/holochain_state/CHANGELOG.md#0.0.1)

## [holochain\_wasm\_test\_utils-0.0.1](crates/holochain_wasm_test_utils/CHANGELOG.md#0.0.1)

## [holochain\_p2p-0.0.1](crates/holochain_p2p/CHANGELOG.md#0.0.1)

## [holochain\_cli\_bundle-0.0.1](crates/holochain_cli_bundle/CHANGELOG.md#0.0.1)

## [holochain\_types-0.0.1](crates/holochain_types/CHANGELOG.md#0.0.1)

### Changed

- BREAKING: All references to `"uuid"` in the context of DNA has been renamed to `"uid"` to reflect that these IDs are not universally unique, but merely unique with regards to the zome code (the genotype) [\#727](https://github.com/holochain/holochain/pull/727)

## [holochain\_keystore-0.0.1](crates/holochain_keystore/CHANGELOG.md#0.0.1)

## [holochain\_sqlite-0.0.1](crates/holochain_sqlite/CHANGELOG.md#0.0.1)

## [kitsune\_p2p-0.0.1](crates/kitsune_p2p/CHANGELOG.md#0.0.1)

## [kitsune\_p2p\_proxy-0.0.1](crates/kitsune_p2p_proxy/CHANGELOG.md#0.0.1)

## [kitsune\_p2p\_transport\_quic-0.0.1](crates/kitsune_p2p_transport_quic/CHANGELOG.md#0.0.1)

## [kitsune\_p2p\_types-0.0.1](crates/kitsune_p2p_types/CHANGELOG.md#0.0.1)

## [kitsune\_p2p\_dht\_arc-0.0.1](crates/kitsune_p2p_dht_arc/CHANGELOG.md#0.0.1)

## [kitsune\_p2p\_mdns-0.0.1](crates/kitsune_p2p_mdns/CHANGELOG.md#0.0.1)

## [mr\_bundle-0.0.1](crates/mr_bundle/CHANGELOG.md#0.0.1)

## [holochain\_util-0.0.1](crates/holochain_util/CHANGELOG.md#0.0.1)

## [hdk-0.0.101](crates/hdk/CHANGELOG.md#0.0.101)

### Changed

- Added `HdkT` trait to support mocking the host and native rust unit tests

### Added

- Added `sign_ephemeral` and `sign_ephemeral_raw`

## [hdk\_derive-0.0.3](crates/hdk_derive/CHANGELOG.md#0.0.3)

## [holochain\_zome\_types-0.0.3](crates/holochain_zome_types/CHANGELOG.md#0.0.3)

### Changed

- `Signature` is a 64 byte ‘secure primitive’

## [holo\_hash-0.0.3](crates/holo_hash/CHANGELOG.md#0.0.3)

## [fixt-0.0.3](crates/fixt/CHANGELOG.md#0.0.3)

### Changed

- Named bytes fixturators like `SixtyFourBytes` are now fixed length arrays

### Added

- Added `SixtyFourBytesVec` to work like the old `Vec<u8>` implementation

# \[20210304.120604\]

This will include the hdk-0.0.100 release.

## [hdk-0.0.100](crates/hdk/CHANGELOG.md#0.0.100)

### Changed

- hdk: fixup the autogenerated hdk documentation.

# 20210226.155101

This release was initiated for publishing the HDK at version *0.0.100-alpha.1*. We are in the process of redefining the release process around this repository so rough edges are still expected at this point.

### Added

- Added App Validation workflow that runs app validation as authority [\#330](https://github.com/holochain/holochain/pull/330)
- Added validation package to entry defs see for usage [\#344](https://github.com/holochain/holochain/pull/344)
- Implemented the `emit_signals` host function [\#371](https://github.com/holochain/holochain/pull/371), which broadcasts a signal across all app interfaces (fine-grained pub/sub to be done in future work)
- get\_details on a HeaderHash now returns the updates if it’s an entry header
- call host fn (This is an actual function not a macro). Allows you to call a zome that is installed on the same conductor. [\#453](https://github.com/holochain/holochain/pull/453)
- Added create link HeaderHash to the Link type
- `remote_signal` host function to send a signal to a list of agents without blocking on the responses. See [\#546](https://github.com/holochain/holochain/pull/546) or the docs for the hdk.
- `hc` utility. Work with DNA and hApp bundle files, set up sandbox environments for testing and development purposes, make direct admin calls to running conductors, and more.

### Changed

- BREAKING: get\_details and get\_links\_details return SignedHeaderHashed instead of the header types [\#390](https://github.com/holochain/holochain/pull/390)
- BREAKING: ZomeInfo now returns the ZomeId [\#390](https://github.com/holochain/holochain/pull/390)
- BREAKING: HoloHash now serializes as a plain 39-byte sequence, instead of a `{hash, hash_type}` structure [\#459](https://github.com/holochain/holochain/pull/459)
- BREAKING: (Almost) all HDK functions have been converted from macros to functions [\#478](https://github.com/holochain/holochain/pull/478)
- Admin interface method `install_app` has its `app_id` field renamed to `installed_app_id` so as not to conflict with the future concept of an “app id”
- Admin interface method `list_active_app_ids` renamed to `list_active_apps`
- BREAKING: JSON replaced with YAML for DNA Properties as well as the DNA manifest (dna.yaml instead of dna.json) [\#592](https://github.com/holochain/holochain/pull/592)
- BREAKING: format of AppInfo changed

### Removed

- The `dna_util` has absorbed by the new `hc` utility.

### Fixed

- If installing the same app\_id twice, previously the second installation would overwrite the first. Now it is an error to do so.
