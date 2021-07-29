# Changelog

This file conveniently consolidates all of the crates individual CHANGELOG.md files and groups them by timestamps at which crates were released. The file is updated every time one or more crates are released.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# \[Unreleased\]

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
