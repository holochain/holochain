# Changelog

This file conveniently consolidates all of the crates individual CHANGELOG.md files and groups them by timestamps at which crates were released. The file is updated every time one or more crates are released.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# \[Unreleased\]

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

## Global

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
