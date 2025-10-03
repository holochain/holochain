---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- Removed the hdk host function `version`, which was not implemented.
- Removed the following public functions, types and enum variants that were not implemented or not used:
  - `holochain_cascade`
    - `CascadeError::RecordGroupError`
  - `holochain_integrity_types`
    - `UpdateAction`
    - `DeleteAction`
    - `agent_state_mut`
    - `signature_mut`
    - `WeighInput`
    - `Record::as_entry_mut`
    - `RequiredValidationType`
  - `holochain_sqlite`
    - `_check_migrations`
    - `DatabaseError::EnvironmentDoubleInitialized` \
    - `DatabaseError::NoPrivateDb`
  - `holochain_state`
    - `SourceChainError::MissingHead`
    - `SourceChainError::MalformedEntry`
    - `SourceChainError::InvalidPreviousAction`
    - `SourceChainError::InvalidLink`
    - `SourceChainError::ScratchNotFresh`
    - `SourceChainError::RecordMissing`
    - `SourceChainError::RecordGroupError`
    - `unset_withhold_publish`
    - `Scratch::num_actions`
  - `holochain_types`
    - `InstalledCell::into_id`
    - `InstalledCell::into_role_name`
    - `InstalledCell::as_id`
    - `InstalledCell::as_role_name`
    - `AutonomicProcess`
    - `first`
    - `first_ref`
    - `second`
    - `second_ref`
    - `swap2`
    - `transpose`
    - `DhtOpLite::fetch_dependency_hashes`
    - `produce_op_lites_from_record_group`
    - `produce_op_lites_from_parts`
    - `RecordGroupError`
    - `ZomeTypesError::MissingZomeType`
    - `GetLinksResponse`
    - `WireRecord`
    - `RecordGroup`
    - `GetRecordResponse`
    - `RawGetEntryResponse`
    - `FakeProperties`
  - `holochain_zome_types`
    - `DeterministicGetAgentActivityFilter`
    - `DeterministicGetAgentActivityResponse`
    - `InlineZomeError::NoSuchCallback`
    - `EntryDhtStatus::Pending`
    - `EntryDhtStatus::Rejected`
    - `EntryDhtStatus::Abandoned`
    - `EntryDhtStatus::Conflict`
    - `EntryDhtStatus::Withdrawn`
    - `EntryDhtStatus::Purged`
    - `ZomeApiVersion`
  - `holochain`
    - `TaskManagerError::TaskManagerFailedToStart`
    - `TaskManagerClient::stop_all_tasks`
    - `PendingJoinReason`
    - `ConductorApiError::ZomeCallCellMismatch`
    - `EntryDefStoreError::TooManyEntryDefs`
    - `ConductorError::CellNotInitialized`
    - `ConductorError::ConfigError`
    - `ConductorError::AppInterfaceIdCollision`
    - `ConductorError::AppNotRunning`
    - `ConductorError::NoConfigPath`
    - `ConductorError::MissingTrigger`
    - `RibosomeError::InvalidCloneTarget`
    - `check_spam`
    - `ValidationDependencies::get_network_fetched_hashes`
  - `hc_sandbox`
    - `parse_dnas`
  - `holochain_keystore`
    - `spawn_real_or_mock_keystore`
    - `RealOrMockKeystore`
    - `MockLairControl`
  - `hc_demo_cli`
    - `BUILD_MODE`
  - `holochain_chc`
    - `records_from_actions_and_entries`
  - `holochain_util`
    - `run_on`


## 0.6.0-dev.25

- Update the Lair keystore dependency to require 0.6.3, which is not a functional change but eliminates multiple unmaintained dependencies. \#5317
- **BREAKING CHANGE**: Dependency updates mean that Holochain now requires Rust 1.88 or later. \#5317

## 0.6.0-dev.24

- Switch from the unmaintained `structopt` library to the maintained `clap` library in the Holochain conductor CLI. \#5316
- **BREAKING CHANGE**: Remove the NetId hash types from the `holo_hash` crate. These were added speculatively and are not used for anything. \#5306
- Remove the NetIdHash types from the `holo_hash` crate. These were added speculatively and are not used for anything. \#5306
- Permit `must_get_valid_record` to retrieve data from the network when called from a coordinator zome. \#5304
- Remove agents from peer store when they are blocked.
- Refactor HDK call `block_agent` to use `HolochainP2pActor::block`.

## 0.6.0-dev.23

- Support binding admin and app websocket interfaces on designated listen addresses. \#5271
- Remove obsolete kitsune\_p2p types that were used with the previous version of kitsune.
- Add tests for filtered `AgentInfo` calls. \#5293

## 0.6.0-dev.22

- Add query `are_all_blocked`, taking a vector of `BlockTargetId`s, to `holochain_state`.
- Upgrade to kitsune2 0.3.0.
- Deprecate `BlockTarget::NodeDna` and `BlockTarget::Node`. These block variants are not currently respected and will be removed with the 0.6.0 minor release.
- Add `Blocks` implementation to `holochain_p2p`, allowing for managing network blocks.

## 0.6.0-dev.21

- Validate warrants in the sys validation workflow. \#5249
- Upgrade `syn` to `2.0` and `darling` to `0.21.3`. \#4446
- Mark `schedule` host fn as stable in code. \#5240
- Upgrade `strum` and `strum_macros` to `0.27.x`. \#4447
- Add tests for scheduling persisted functions across multiple cells.
- Remove unused enum variant `Infallible` from `StateMutationError`. \#5270

## 0.6.0-dev.20

- Add a new `copy_cached_op_to_dht` function to `holochain_state`. \#5252
- Add a new function `get_dht_op_validation_state` to `holochain_state` for checking the state of a DHT op. \#5246
- Fix a reference to a method that no longer exists in the conductor documentation `list_dnas` -\> `list_dna_hashes`. \#5245
- Remove generic type parameters in `SysValDeps` and related types that are always used with the default types. \#5245
- Add tests for `publish_query` to ensure that chain ops and warrant ops pending publish are counted correctly.

## 0.6.0-dev.19

- Changed holochain\_metrics dashboards to match available metrics.
- Internal refactor to remove the `same_dht` field of `SysValDeps`. This field was redundant because the `SysValDeps` are always for the same DHT as the cell they are part of. \#5243
- **BREAKING CHANGE**: The agent activity response has been changed to return warrants as a `Vec<SignedWarrant>` instead of a `Vec<Warrant>`. This change ensures that warrant integrity can be checked and discovered warrants can be validated. Note that this also affects the HDK’s `get_agent_activity` function which will now also return `SignedWarrant`s instead of `Warrant`s. \#5237
- **BREAKING CHANGE**: Move `ChainOpType` from `holochain_types` to `holochain_zome_types`. \#5236
- **BREAKING CHANGE**: Remove the `SysValDeps` typedef and use instead `Vec<ActionHash>` in the few places that was used. \#5236
- **BREAKING CHANGE**: Modify the fields of `ChainIntegrityWarrant::ChainIntegrityWarrant` to add a `ChainOpType` field which allows just one op type to be validated when checking the warrant. \#5236
- Changed `schedule` host fn unit tests to integration tests.
- **BREAKING CHANGE**: Deprecate `AppManifest::UseExisting`. For late binding, update the coordinators of a DNA. For calling cells of other apps, bridge calls can be used.
- Fix: Unschedule already scheduled persisted functions on error or when the schedule is set to `None`.
- Refactor: When representative agent is missing, skip app validation workflow instead of panicking.
- Fix: Return an error with an `Invalid` result from `must_get_valid_record` when a record that is invalid is found. Previously the error returned indicated `UnresolvedDependencies`.
- Feat: Include warrants in publish and gossip and enable them to be fetched.
- Fix: Correctly return metrics when calling `dump_network_metrics_for_app` with apps that have clone cells.

## 0.6.0-dev.18

- Malformed websocket requests are responded to with an error ([\#5209](https://github.com/holochain/holochain/pull/5209)).
- Add support for writing metrics to InfluxDB file on disk.
- Add tests for the `schedule` host fn
- Fix: `hc-sandbox run` dedupes indices before proceeding.
- Update Kitsune2 to 0.2.15 and tx5 to 0.7.1 to get various bugfixes.
- As part of the tx5 update, go-pion has been upgraded to v4, and as part of the Kitsune2 update, there are now features to allow either libdatachannel or go-pion to be used as the WebRTC backend. The default is still libdatachannel.
- **BREAKING CHANGE**: Deprecate `RoleSettings::UseExisting`. For late binding, update the coordinators of a DNA. For calling cells of other apps, bridge calls can be used.
- **BREAKING CHANGE**: Host function `call` returns a specific error when calling another cell by cell ID that cannot be found.

## 0.6.0-dev.17

- Move the logic of adding `DnaFile`s to the dna files cache upon installing apps in a `SweetConductor` up to the `SweetConductor`’s `install_app()` method to ensure that `DnaFile`s also get added to the cache if the `install_app()` method is being used directly.
- Add an `update_coordinators()` method to the `SweetConductor` that also updates the `DnaFile` associated with the cell whose coordinators got updated in the `SweetConductor`’s dna files cache.
- **BREAKING CHANGE** The field `dna_hash: DnaHash` in the `UpdateCoordinatorsPayload` of the `UpdateCoordinators` admin call is replaced with a field `cell_id: CellId` ([\#5189](https://github.com/holochain/holochain/pull/5189)).
- **BREAKING CHANGE** The admin call `GetDnaDefinition` now takes a `CellId` as argument instead of a `DnaHash` because there can be two identical DNAs for different agents in the conductor and they were not looked up correctly prior to this change ([\#5189](https://github.com/holochain/holochain/pull/5189)).
- Fixed issue [\#2145](https://github.com/holochain/holochain/issues/2145) in ([\#5189](https://github.com/holochain/holochain/pull/5189)) by
  - indexing Ribosomes by cell id instead of by dna hash in the in-memory RibosomeStore
  - indexing DnaFiles by cell id instead of by dna hash in the DnaDef database on disk
  - fixing the update\_coordinators() method in the conductor and the associated SQL query to actually update DnaDef’s in the database if a DnaDef already exists in the database for the given cell id.
- Panic when attempting to bundle a dna from a DnaFile with inline zomes or when attempting to construct a dna manifest from a DnaDef with inline zomes ([\#5185](https://github.com/holochain/holochain/issues/5185)).
- Refactor chain head coordinator related tests to not rely on the ability to install dnas by specifying the `installed_hash` in the manifest only ([\#5185](https://github.com/holochain/holochain/issues/5185)).
- **BREAKING CHANGE** Remove support for installing a dna (as part of an app) only by specifying an `installed_hash` and without bundling the actual dna code ([\#5185](https://github.com/holochain/holochain/issues/5185)).

## 0.6.0-dev.16

- Removed redundant usages of `register_dna()` in test code and removed unused test utils ([\#5174](https://github.com/holochain/holochain/pull/5174)).
- Fixes the type of the `expires_at` field in `PeerMetaInfo` returned by the admin API from `Timestamp` to `Option<Timestamp>` ([\#5183](https://github.com/holochain/holochain/pull/5183)).
- **BREAKING CHANGE**: The admin call `RegisterDna` has been removed ([\#5175](https://github.com/holochain/holochain/pull/5175))
- As part of the fix below, the Holo hash method `to_k2_op` on a DhtOpHash` has been deprecated and replaced with  `to\_located\_k2\_op\_id\`.
- Fixes a bug where the wrong DhtOp location was reported to Kitsune2. This resulted in conductors not being able to sync with each other. This change can upgrade existing conductors and new data should sync correctly. However, part of the DHT model gets persisted and to fix bad data in the persisted model, the model has to be wiped and rebuilt. This will result in a short startup delay when upgrading to this version. After the first startup, the startup time should be back to normal.
- **BREAKING CHANGE**: `hc-sandbox` API and behavior changes:
  - Remove `--existing-paths` and `--last` options.
  - When `hc s run <INDICES>` and a sandbox folder from `.hc` is missing, command will abort and display an error message.
  - When calling `hc s run --all` and a sandbox folder from `.hc` is missing, command will proceed and display a warning for each missing sandbox.
  - When the `.hc` file is missing, the command will abort and display an error message.
  - Add new command `hc s remove <INDICES>`
  - `hc s clean` displays a short message upon completion telling how many paths were removed.
  - `hc s list` displays a specific message when there are no sandboxes.

## 0.6.0-dev.15

- Replace `Conductor::remove_dangling_cells` method with methods that remove the cells specific to the app and delete their databases.
- **BREAKING CHANGE**: Remove unused field `ConductorBuilder::state`.
- Remove network joining timeout. This used to work with the previous version of kitsune, but now all the `join` call does is to join the local peer store which is a matter of acquiring a write lock on a mutex and doesn’t indicate whether publishing the agent info to the peer store and the bootstrap has been successful.
- **BREAKING CHANGE**: Remove unused function `get_dependency_apps`.
- **BREAKING CHANGE**: Rename `AgentMetaInfo` to `PeerMetaInfo` since the info returns meta data for potentially multiple agents at a given peer URL ([\#5164](https://github.com/holochain/holochain/pull/5164)).
- **BREAKING CHANGE**: `hc-sandbox call` now returns structured data as JSON.

## 0.6.0-dev.14

- Refactor conductor methods `enable_app`, `disable_app`, `uninstall_app` and `initialize_conductor` to directly manage cells instead of using state machine code.
- **BREAKING CHANGE**: `AdminRequest::EnableApp` fails when creating the app’s cells fails and returns the first error that occurred. In case of success the enabled app info is returned.
- Remove state machine functions from conductor, which have been replaced by functions that process the necessary steps directly.
- **BREAKING CHANGE**: Use `AppStatus` in favor of `AppInfoStatus` in `AppResponse::AppInfo`.
- Remove app status transition functions and `AppInfoStatus`.
- **BREAKING CHANGE**: Remove types `EnabledApp` and `DisabledApp` in favor of `InstalledApp` to reduce app handling complexity.
- **BREAKING CHANGE**: Replace and remove legacy constructor for `InstalledAppCommon`.
- Remove an unnecessary use of `DnaFile` in the genesis workflow ([\#5150](https://github.com/holochain/holochain/pull/5150)).

## 0.6.0-dev.13

- **BREAKING CHANGE**: Remove `pause_app` and `Paused` state from conductor. Pausing an app was used when enabling an app partially failed and could be re-attempted. Now the app is only enabled if all cells successfully started up.
- **BREAKING CHANGE**: Removed everything related to `started` and `stopped` apps. Instead `enabled` and `disabled` remain as the only two possible states an app can be in after it has been installed.
- **BREAKING CHANGE**: Remove `CellStatus` which used to indicate whether a cell has joined the network or not. Going forward cells that couldn’t join the network will not be kept in conductor state.
- **BREAKING CHANGE**: Remove `generate_test_device_seed` from `ConductorBuilder`. This was a remnant from DPKI.
- Add integration tests for the use of `path`’s and the links created by them. ([\#5114](https://github.com/holochain/holochain/pull/5114))
- **BREAKING CHANGE** Removed an if/else clause in the `Ribosome::new()` impl that lead to inconsistent behavior depending on the number of zomes defined in the dna manifest ([\#5105](https://github.com/holochain/holochain/pull/5105)). This means that **the dependencies field for zomes in the dna manifest is now always mandatory**. Previously, if there was only one single integrity zome in the whole dna, it was implied by the conductor that a coordinator zome would depend on that integrity zome. This is no longer the case.
- Clearer error message if `ScopeLinkedType` or `ScopedEntryDefIndex` cannot be created due to zome dependencies not being specified in the dna manifest ([\#5105](https://github.com/holochain/holochain/pull/5105)).
- Added a new Admin API endpoint to revoke zome call capability `revoke_zome_call_capability`. [Issue 4596](https://github.com/holochain/holochain/issues/4596)
- **BREAKING CHANGE**: the return type of `capability_grant_info` has been changed from `HashMap<CellId, Vec<CapGrantInfo>>` to `Vec<(CellId, Vec<CapGrantInfo>)>`. This is to make it work with JSON encoding, which does not support maps with non-string tuple keys. The new type is now also used in the Admin API response `CapabilityGrantsInfo`.
- **BREAKING CHANGE**: the return type of `grant_zome_call_capability` has been changed from `()` to `ActionHash`. This has been done to make it easier to know the `ActionHash` of the grant in case you want to revoke it later.

## 0.6.0-dev.12

- It’s possible to configure an advanced setting for the network layer that shows tracing information about network connectivity and state changes. Rather than having to configure that in Holochain runtimes, it is now automatically enabled when the `NETAUDIT` tracing target is enabled at `WARN` level or lower.
- Test app operations (install/enable/disable/uninstall) with regards to app state and cell state.
- **BREAKING CHANGE**: Remove `start_app` from conductor. Use `enable_app` instead.
- Remove unused field `dna_def` the `HostFnWorkspace` struct ([\#5102](https://github.com/holochain/holochain/pull/5102))
- Replace the `dna_def` field of the `SysValidationWorkspace` with a `dna_hash` field.
- Remove unnecessary uses of `LinkType` and `EntryDefIndex` types

## 0.6.0-dev.11

## 0.6.0-dev.10

- Update `AgentInfo` call in `hc-sandbox` to take an optional vector of DNA hashes instead of one optional cell ID, e. g. `hc s call list-agents --dna uhC0kScOJk1aw9a5Kc8jqs0jieGCi4LoeW7vUehsTLvMI455-2hF1 --dna uhC0kScOJk1aw9a5Kc8jqs0jieGCi4LoeW7vUehsTLvMI455-2hF1`.

- Patch `serde_json` to enable byte arrays to be converted to JSON arrays.

- Add `AgentMetaInfo` calls to the conductor’s app and admin interfaces to retrieve data from the peer meta store for a given agent by their Url [\#5043](https://github.com/holochain/holochain/pull/5043)

## 0.6.0-dev.9

- Implement new call `get_all_by_key` for `PeerMetaStore`. Adds the call to retrieve all URLs and values for a given key from the store.

- Filter out unresponsive agents when publishing ops. Any URL that is set as unresponsive in the peer meta store will be filtered out when determining the agents near a location to publish to.

- Change versions of `DnaManifest`, `AppManifest` and `WebAppManifest` to `0`.

## 0.6.0-dev.8

## 0.6.0-dev.7

- Add `AgentInfo` call to the conductor’s app interface to retrieve the discovered peers of the app’s various DNAs.
- **Breaking**: admin interface `AgentInfo` call now takes an optional list of DNA hashes to filter by instead of a `CellId`.
- Add methods to the peer meta store that mark a peer URL as unresponsive and check if a peer URL is marked unresponsive. This change enables marking peers as unresponsive when connecting to or sending messages to, so that they can be omitted from future requests until the URL expires.

## 0.6.0-dev.6

- Bump holochain-wasmer and wasmer to v6.

## 0.6.0-dev.5

- Adds the `--origin` option to the `hc sandbox call` command in order to allow making admin calls to deployed conductors with restricted `allowed_origins`.

## 0.6.0-dev.4

- Change `hc-sandbox` to use admin and app clients from the `holochain_client` crate
- Change `hcterm` to use the admin and app websocket clients from `holochain_client` internally
- Reinstate indexes on `DhtOp` tables. With the latest migration script, the indexes were not carried over. \#4970
- Optimize `ChainHeadQuery` for performance. A flame graph analysis revealed that a significant proportion of the CPU time was spent on this query in a test with a high number of entry creates and reads. The query now runs about 30 % faster. \#4971
- Allow queries in `holochain_state` to run without parameters. Previously such queries would not be run against any stores. That is now possible.
- Further optimize `ChainHeadQuery` for performance by removing redundant WHERE clause. Formerly multiple agents on a conductor would share the same authored database, so the chain head had to be determined by agent. Since a refactor every agent has their own authored database, thus the clause is no longer required. \#4974
- Remove unused column `dependency` and related indexes from table `DhtOp`.
- Remove placeholder variant `AppResponse::AppAgentKeyRotated`.

## 0.6.0-dev.3

- New feature to permit using a `call` from the `post_commit` hook. This was previously only possible with a `remote_call` to yourself. That no longer works because the networking doesn’t let you “connect” to yourself. This is a shorter and clearer route to the same result. \#4957

## 0.6.0-dev.2

- Integrate all app validated ops, regardless of type and order
- Remove DHT DB query cache, a premature optimization that made integration complicated and error-prone without any appreciable benefit.
- Fix warrant persistence. Warrants were formerly stored in the Action table, but did not quite fit there. A new Warrant table was created, to which warrants are now written.
- Fix bug where a zero-arc node could not delete a link it hasn’t created. Fetching the action that created the link allows network access now.
- Expose `GetOptions` for `delete_link` call. Developers can specify whether `delete_link` should be restricted to local data to look up the create link record, or if it can request it from the network.

## 0.6.0-dev.1

- Refactor Mr. Bundle which affects the manifest types in `holochain_types`.
- **BREAKING** Existing app bundles cannot be used. You can either rebundle existing happs with `hc-bundle` and the `--raw` flag or issue a new version of your app, packaged for Holochain 0.6.
- Add `signalAllowPlainText: true` to conductor configs generated by `hc-sandbox`
- Remove unused `check_chain_rollback` function from `sys_validation`.
- Remove unused `is_chain_empty` and `action_seq_is_empty` functions from `sys_validation_workflow`.
- Change the system validation workflow to never directly put ops to integrated, instead mark them as awaiting integration.
- Refactor: rename status parameter to stage when setting validation stage
- Refactor sweettest function `await_consistency` after seeing it was not behaving reliably. Rewritten to compare all involved peer DHT databases and check if the sum of all ops has been integrated in all of them.

## 0.6.0-dev.0

- Remove unstable feature DPKI and all references to it, including the DPKI conductor service. DPKI was introduced to the codebase prematurely, largely untested, provided only a minimal set of calls to query agent keys and revoke an agent key, and kept causing problems that were disproportionately difficult to debug. If Holochain will be enhanced by agent key management in the future, it will be reimplemented from the ground up.
- Remove all references to Deepkey DNA.
- Remove call to revoke an agent key from the Conductor API. There was no way to update an agent key or create a new key for an agent. The call was a remnant of the intended DPKI feature.
- Remove conductor service interface and app store service stub. The conductor service interface was added primarily to allow DPKI to run as a DNA internal to the conductor.

## 0.5.0

## 0.5.0-rc.1

- ***BREAKING*** Kitsune2 Integrated: Holochain has now transitioned from the legacy Kitsune networking implementation, to the new [Kitsune2](https://crates.io/crates/kitsune2). [\#4791](https://github.com/holochain/holochain/pull/4791)
  - breaking protocol changes
  - differences in the networking section of the conductor config
  - changes to agent info encoding
  - transition from hc run-local-services to kitsune2 bootstrap server
  - switch from go pion to libdatachannel as the WebRTC backend. Holochain no longer depends on the go compiler, but has some additional c++ toolchain dependencies (such as cmake).
- Set crypto provider for TLS connections in Holochain binary. This used to produce error when running the binary.

## 0.5.0-rc.0

- Moves the `lineage` field of the dna manifest behind an `unstable-migration` feature.
- The Admin Api call `GetCompatibleCells` is now only available with the `unstable-migration` feature.

## 0.5.0-dev.22

- Update to Lair 0.6.0
- Changed the response format for `DumpNetworkMetrics` and `DumpNetworkStats` to provide information from Kitsune2. \#4816
- Added a new option to the `DumpNetworkMetrics` on the admin interface. That is `include_dht_summary` which will return a summary of the DHT state. \#4816
- Added `DumpNetworkMetrics` and `DumpNetworkStats` to the app interface. Please see the Rust docs for usage. \#4816
- Removed `NetworkInfo` from the app interface, please use `DumpNetworkMetrics` or `DumpNetworkStats` instead. \#4816
- Update `hcterm` to work with bootstrap2 bootstrap servers. By default, it uses `https://dev-test-bootstrap2.holochain.org` as the bootstrap2 server. \#4767
- Update `hc-service-check` to check bootstrap2 servers. By default, it uses `https://dev-test-bootstrap2.holochain.org` as the bootstrap2 server. \#4767
- Remove `hc-run-local-services`, please use `kitsune2-bootstrap-srv` instead.
- Handle empty databases in `StorageInfo` request. Previously, if the database was empty, the request would return an error. \#4756
- Add `DnaHash` to the `DnaStorageInfo` which is part of the `StorageInfo` response.
- The `agent_latest_pubkey` field of `AgentInfo` is put behind the `unstable-dpki` feature flag ([\#4815](https://github.com/holochain/holochain/pull/4815)).

## 0.5.0-dev.21

## 0.5.0-dev.20

## 0.5.0-dev.19

- Break dependency from holochain\_state to holochain\_p2p
- remove `serde(flatten)` attributes from certain enum variants of enums used in admin payloads (\#4719), thereby fixing an oversight of \#4616.

## 0.5.0-dev.18

- Change most enums that are exposed via the conductor API to be serialized with `tag = "type"` and `content = "value"` \#4616
- Replace `tiny-keccak` with `sha3` due to dependency on problematic `crunchy` crate
- Use `rustls-tls` instead of `native-tls-vendored` in reqwest due to compatibility issue with Android platform
- Prevent “TODO” comments from being rendered in cargo docs.

## 0.5.0-dev.17

- added admin\_api capability\_grant\_info for getting a list of grants valid and revoked from the source chain
- create an independent `Share` type in the holochain crate in order to not depend on the one from kitsune\_p2p

## 0.5.0-dev.16

- Remove the integration step from the app validation and authored ops workflows. Instead, integration is only handled by the integrate workflow.
- Add integration of `StoreRecord` and `StoreEntry` ops to integrate workflow.
- The integrate workflow now integrates **all** valid `RegisterAddLink` ops instead of only ones that link to a `StoreEntry`.
- Update doc-comment about CreateLink Action
- Fix issue where genesis actions weren’t integrated when others were ready to integrate. When nothing had been integrated yet then we started integration at the value of how many ops were `ready_to_integrate` so if we had other ops that were ready then the range started at them instead of at genesis (index 0).
- Update `await_consistency` test utility function so that it prints every inconsistent agent when it fails instead of just the first one.
- Rename the SQL queries that are used to set `RegisterAddLink` and `RegisterRemoveLink` ops to integrated
- Add smoke test for `AdminRequest::DumpConductorState`
- Fix issue where `AdminRequest::DumpConductorState` fails when the conductor has an `AppInterface` attached.

## 0.5.0-dev.15

- Update `holochain_wasmer_common`.

## 0.5.0-dev.14

- Remove support for x86\_64-darwin in Holonix. This is becoming hard to support in this version of Holonix. If you are relying on support for a mac with an Intel chip then please migrate to the new [Holonix](https://github.com/holochain/holonix?tab=readme-ov-file#holonix)
- Add two new commands to the `hc sandbox` for authenticating and making zome calls to a running conductor. See the sandbox documentation for usage instructions. \#4587
- Update `holochain_wasmer_host`, remove temporary fork of wasmer and update wasmer to 5.x.
- Disable wasmer module caching when using the feature flag `wasmer_wamr`, as caching is not relevevant when wasms are interpreted.
- Add a `--create-config` flag to handle config generation
- Add a `--config-schema` flag to `holochain` that prints out a json schema for the conductor config.

## 0.5.0-dev.13

- Added LinkTag helper trait functions to go into and out with serialized bytes

## 0.5.0-dev.12

## 0.5.0-dev.11

- Prevent duplicate calls to `init`. Previously, calling `init` directly would result in 2 calls to `init` if the zome had not been initialised yet and 1 call if the zome had been initialised. Now, calling `init` directly will result in 1 call by the conductor to initialise the zome and all subsequent calls to `init` will return `InitCallbackResult::Pass`. This both fixes surprising behavior and allows `init` to be called directly to initialise a zome if desired.

## 0.5.0-dev.10

## 0.5.0-dev.9

## 0.5.0-dev.8

- made holo\_hash encoding default in hdk cargo.toml for common B64 hashes

## 0.5.0-dev.7

- **BREAKING**: The `InstallAppPayload` now unifies all settings that are per role in a `roles_settings` field and as part of this change adds the option to specify custom modifiers at install time to override the modifiers defined in the dna manifest(s).
- Zome call authorization is split into autentication and authorization. Zome calls are authenticated when coming in over the network. The signature must match the hash of the serialized bytes, signed by the provenance of the call, as described above. This applies to zome calls over the App API as well as remote calls. Bridge calls, which are calls that zome call functions can make to other cells on the same conductor, do not require authentication. Authorization through zome call capabilities remains unchanged and is required for any kind of call as before.
- Remove `release-automation` crate from the `cargo update` script. `release-automation` isn’t used externally so compatibility with all dependency versions isn’t required and it often causes the job to fail.

## 0.5.0-dev.6

- **BREAKING**: Zome call API `AppRequest::CallZome` takes simple serialized bytes of the zome call parameters and the signature now. Previously client-side serialization of zome call parameters required to exactly match Holochain’s way of serializing, because Holochain re-serialized the parameters to verify the signature. This is no longer the case. The signature is generated for the **hash of the serialized bytes**, using the **SHA2 512-bit** hashing algorithm. In short, zome call params are serialized, then hashed and the hash is signed. The payload of the `CallZome` request is the serialized bytes and the signature. On the Holochain side the serialized bytes of the zome call parameters are hashed with the same SHA2 512-bit algorithm to verify the signature.

## 0.5.0-dev.5

- **BREAKING** Countersigning has been put behind the feature `unstable-countersigning`. Even though in many use cases countersigning is expected to work correctly, it has known problems which can put the source chain into an unrecoverable state. Included in this feature is the HDK function `accept_countersigning_preflight_request` as well as `AppRequest`s related to countersigning and the counersigning workflow itself too.
- **BREAKING** The following HDK functions have been temporarily removed as “unstable”. They can be re-enabled by building Holochain with the “unstable-functions” feature flag:
  - `accept_countersigning_preflight_request`
  - `block_agent`
  - `unblock_agent`
  - `get_agent_key_lineage`
  - `is_same_agent`
  - `schedule`
  - the function `sleep` has been removed entirely because it wasn’t implemented
  - and the HDI function `is_same_agent` Note that installing apps that have been built with an HDK from before this change will not be possible to install on a conductor that has been built without the `unstable-functions` feature. You will get import errors when Holochain tries to compile the WASM. It is valid to install an app that has been compiled without the `unstable-functions` feature onto a conductor which has been compiled with `unstable-functions` but the reverse is not true. \#4371
- Fix a problem with countersigning where it would stay in resolution when entering an unknown state from a restart. This was intended behaviour previously to ensure the agent got a change to get online before giving up on countersigning but it is not necessary now that we consider network errors to be a failed resolution and always retry.

## 0.5.0-dev.4

- **BREAKING**: As the DPKI feature is unstable and incomplete, it is disabled with default cargo features and put behind a feature called `unstable-dpki`. If this feature is specified at compile time, DPKI is enabled by default.
- **BREAKING**: Issuing and persisting warrants is behind a feature `unstable-warrants` now. Warrants have not been tested extensively and there is no way to recover from a warrant. Hence the feature is considered unstable and must be explicitly enabled. Note that once warrants are issued some functions or calls may not work correctly.
- **BREAKING**: Conductor::get\_dna\_definitions now returns an `IndexMap` to ensure consistent ordering.
- Add test to make sure sys validation rejects deleting a delete. Unit tests to get zomes to invoke for app validation were removed for these cases of deleting a delete, because the code path cannot be reached by the system.
- Added a new feature “unstable-sharding” which puts the network sharding behind a feature flag. It will not be possible to configure network sharding unless Holochain is built with this feature enabled. By default, the network tuning parameter `gossip_dynamic_arcs` is ignored, and the parameter `gossip_arc_clamping` must be set to either `"full"` or `"empty"`, the previous default value of `"none"` will prevent the conductor from starting. We intend to stabilise this feature in the future, and it will return to being available without a feature flag. \#4344

## 0.5.0-dev.3

- Use of WasmZome preserialized\_path has been **deprecated**. Please use the wasm interpreter instead.

- Conductor::get\_dna\_definitions now returns an `IndexMap` to ensure consistent ordering.

## 0.5.0-dev.2

- Add App API calls to interact with an unresolvable countersigning session. State of countersigning can be queried with `AppRequest::GetCountersigningSessionState`, an unresolvable session can be abandoned using `AppRequest::AbandonCountersigningSession` or force-published by making `AppRequest::PublishCountersigningSession`. Abandoning and publishing is only possible for sessions that have been through automatic resolution at least once where Holochain has not been able to make a decision. \#4253

## 0.5.0-dev.1

- AdminRequest::ListApps is now sorted by the new AppInfo field `installed_at`, in descending order
- Return a `RibosomeError` when there is a serialisation error invoking a zome callback. For example, if they have an invalid return type or parameters. This error bubbles-up and causes the zome call to fail, giving nicer errors and removing the panic which crashed the conductor in these situations. \#3803

## 0.5.0-dev.0

## 0.4.0

## 0.4.0-dev.28

## 0.4.0-dev.27

- HC sandbox: Fix `--no-dpki` option which previously enabled DPKI in the conductor when set, instead of disabling it.
- Remove the out-dated `validation_callback_allow_multiple_identical_agent_activity_fetches` test. Originally, it was to test that an identical op is only fetched from the network once and then looked up in the cache. After a refactor of production code this was no longer the case and so the test was refactored to check that it can fetch from the network multiple times. There can be no guarantee that it will do one over the other so the test is naturally flaky.
- Update the following tests to add a wait for gossip before creating ops. This adds an extra delay and makes sure that the conductors see each other before continuing with the tests.
  - `multi_create_link_validation`
  - `session_rollback_with_chc_enabled`
  - `alice_can_recover_from_a_session_timeout`
  - `should_be_able_to_schedule_functions_during_session`
- Update Makefile default recipe to use the new recipes that build and test the workspace with the feature flags `wasmer_sys` and `wasmer_wamr`. \#4284
- Add support for parsing the lint-level as a set in the Nix holochain module. e.g. `nursery = { level = "allow", priority = -1 }`. \#4284
- Add the `nix/` directory as a watch point for `direnv` so it reloads the `devShell` if a file changes in that directory. \#4284

## 0.4.0-dev.26

- Countersigning sessions no longer unlock at the end time without checking the outcome. There is a new workflow which will take appropriate actions when the session completes or times out. The majority of the logic is unchanged except for timeouts. If a timeout occurs and Holochain has been up throughout the session then the session will be abandoned. If Holochain crashes or is restarted during the session but is able to recover state from the database, it will attempt to discover what the other participants did. This changes a failure mode that used to be silent to one that will explicitly prevent new writes to your source chain. We are going to provide tooling to resolve this situation in the following change. \#4188
- Internal rework of chain locking logic. This is used when a countersigning session is in progress, to prevent other actions from being committed during the session. There was a race condition where two countersigning sessions being run one after another could result in responses relevant to the first session accidentally unlocking the new session. That effectively meant that on a larger network, countersigning sessions would get cancelled when nothing had actually gone wrong. The rework of locking made fixing the bug simpler, but the key to the fix was in the `countersigning_success` function. That now checks that incoming signatures are actually for the current session. \#4148

## 0.4.0-dev.25

## 0.4.0-dev.24

- Add `danger_generate_throwaway_device_seed` to allow creation and use of a random device seed for test situations, where a proper device seed is not needed. \#4238
- Add `allow_throwaway_random_dpki_agent_key` to allow creation of a random (unrecoverable) DPKI agent when a device seed is not specified. \#4238
- Fixes issue \#3679 where websocket connections would be closed if a message was received that failed to deserialize. The new behaviour isn’t perfect because you will get a timeout instead, but the websocket will remain open and you can continue to send further valid message. There is another issue to track partial deserialization \#4251 so we can respond with an error message instead of a timeout. \#4252

## 0.4.0-dev.23

- Fixes issue \#3679 where websocket connections would be closed if a message was received that failed to deserialize. The new behaviour isn’t perfect because you will get a timeout instead, but the websocket will remain open and you can continue to send further valid message. There is another issue to track partial deserialization \#4251 so we can respond with an error message instead of a timeout. \#4252

## 0.4.0-dev.22

- `device_seed_lair_tag` is now part of `ConductorConfig`. This was previously a field as part of the optional DPKI config. Now, a device seed should be specified even if not using DPKI. If the device seed is specified, when installing an app without providing an agent key, a new agent key will be generated by deriving a key from the device seed using the total number of apps ever installed as part of the derivation path. If the device seed is not specified, it will not be possible to install an app without specifying an agent key (app installation will error out).
- `allow_throwaway_random_agent_key` can be set in the `InstallAppPayload` to override the aforementioned behavior, allowing an agent key to not be specified even if a device seed is not specified in the conductor. This is a safety mechanism, and should only be used in test situations where the generated agent key is a throwaway and will never need to be recovered.
- Holochain now actually makes use of the device seed, which was previously ignored
- Holochain now makes sure to properly register in DPKI any pregenerated agent key which is provided in app installation, when DPKI is enabled.
- **BREAKING:** Modifies `Action::CloseChain` and `Action::OpenChain` to be able to represent both DNA migrations and Agent migrations:
  - CloseChain can be used on its own, with no forward reference, to make a chain read-only.
  - CloseChain can include a forward reference to either a new AgentPubKey or a new DNA hash, which represent a migration to a new chain. The new chain is expected to begin with a corresponding OpenChain which has a backward reference to the CloseChain action. (This will become a validation rule in future work.)
- Internal rework of `get_agent_activity`. This is not a breaking change for the HDK function of the same name, but it is a breaking change to the previous version of Holochain because the network response for agent activity has been changed. A future change will be made to the HDK function to expose the new functionality. \#4221
- Add feature flags `wasmer_sys` and `wasmer_wamr` to toggle between using the current wasm compiler and the new, experimental wasm interpreter. `wasmer_sys` is enabled as a default feature to preserve existing behavior.

## 0.4.0-dev.21

- HDK: Add call to get an agent key lineage. A key lineage includes all keys of an agent that have been generated by creating and updating the key.
- Remove obsolete host context `MigrateAgentHostAccess`. It is not used anywhere.

## 0.4.0-dev.20

- **BREAKING\!** Enables dynamic database encryption. (as opposed to the hard-coded key that was previously being used.) NOTE - this is incompatible with all previous holochain databases, they will not open, and must be deleted. NOTE - this is incompatible with all previous lair databases, they will not open and must be deleted. [\#4198](https://github.com/holochain/holochain/pull/4198)

## 0.4.0-dev.19

- Adds DPKI support. This is not fully hooked up, so the main implication for this particular implementation is that you must be using the same DPKI implementation as all other nodes on the network that you wish to talk to. If the DPKI version mismatches, you cannot establish connections, and will see so as an error in the logs. This work is in preparation for future work which will make it possible to restore your keys if you lose your device, and to revoke and replace your keys if your device is stolen or compromised.
- Add feature to revoke agent keys. A new call `AdminRequest::RevokeAgentKey` is exposed on the Admin API. Revoking a key for an app will render the key invalid from that moment on and make the source chain of all cells of the app read-only. The key is revoked in the Deepkey service if installed and deleted on all cells’ source chains. No more actions can be written to any of these source chains. Further it will fail to clone a cell of the app.
- App validation workflow: Remove tracking of missing dependencies. Tracking them introduces higher complexity and the possibility of ops under validation to be stuck. The delay before re-triggering app validation is increased to 100-3000 ms, giving the background task that fetches missing dependencies a chance to complete.

## 0.4.0-dev.18

## 0.4.0-dev.17

## 0.4.0-dev.16

- App manifest field `membrane_proofs_deferred` renamed to `allow_deferred_memproofs`, and the semantics are changed accordingly: if this field is set and memproofs are not provided at installation time (i.e. None is used), then the app will go into the deferred memproof state. Otherwise, if the field is set and memproofs are provided, installation will proceed as if the field were not set.
- Add HDI call to check if two agent keys are of the same key lineage. It will be possible for an agent to update their key. This new key as well as the old key are part of the same lineage, they belong to the same agent. With the new HDI call `is_same_agent`, app validation can check if two agent keys belong to the same agent. Key updates are exclusive to conductors with a DPKI service installed. If DPKI is not installed. `is_same_agent` compares the two provided keys for equality.
- Adds the [`UseExisting`](https://github.com/holochain/holochain/blob/293d6e775b3f02285b831626c9911802207a8d85/crates/holochain_types/src/app/app_manifest/app_manifest_v1.rs#L155-L165) cell provisioning strategy, an alternative to `Create`, allowing an app to depend on a cell from another installed app. Read the rustdocs for more info on this new type of provisioning.
- Possible performance improvement: better async handling of wasm function calls which should allow more concurrent throughput system during long-running zome calls \#4111
- New protections are put in place for apps which are depended upon by other apps via `UseExisting`. Any “protected” inter-app dependency will prevent a dependency app from being uninstalled until the dependent app is also uninstalled, or if the `force` parameter is set to true in the `UninstallApp` call.
- CountersigningSuccess signal that is emitted when a countersigning session is successfully completed now includes the
- *BREAKING* Introduced a new workflow error, `IncompleteCommit`. When inline validation fails with missing dependencies. I.e. Validation for actions that are being committed to the source chain during a zome call discovers missing dependencies. The generic `InvalidCommit` is replaced by this new error. That allows the caller to distinguish between errors that are fatal and errors that can be retried. For now, the only retryable error is caused by missing dependencies. \#4129
- Based on the change above, about adding `IncompleteCommit`, a countersigning session will no longer terminate on missing dependencies. You may retry committing the countersigned entry if you get this error. \#4129
- *BREAKING* CountersigningSuccess signal that is emitted when a countersigning session is successfully completed now includes the `app_entry_hash` from the `PreflightRequest` rather than the `EntryHash` that is created when you commit the countersigned entry. This value is easier for clients to get at and use to check that the countersigning session they joined has succeeded. \#4124

## 0.4.0-dev.15

- *BREAKING* Introduced a new workflow error, `IncompleteCommit`. When inline validation fails with missing dependencies. I.e. Validation for actions that are being committed to the source chain during a zome call discovers missing dependencies. The generic `InvalidCommit` is replaced by this new error. That allows the caller to distinguish between errors that are fatal and errors that can be retried. For now, the only retryable error is caused by missing dependencies. \#4129
- Based on the change above, about adding `IncompleteCommit`, a countersigning session will no longer terminate on missing dependencies. You may retry committing the countersigned entry if you get this error. \#4129
- *BREAKING* CountersigningSuccess signal that is emitted when a countersigning session is successfully completed now includes the `app_entry_hash` from the `PreflightRequest` rather than the `EntryHash` that is created when you commit the countersigned entry. This value is easier for clients to get at and use to check that the countersigning session they joined has succeeded. \#4124

## 0.4.0-dev.14

## 0.4.0-dev.13

## 0.4.0-dev.12

- When uninstalling an app or removing a clone cell, only some of the data used by that cell was deleted. Now all data is deleted, freeing up disk space.
- Adds a new `DisabledAppReason::NotStartedAfterProvidingMemproofs` variant which effectively allows a new app status, corresponding to the specific state where a UI has just called `AppRequest::ProvideMemproofs`, but the app has not yet been Enabled for the first time.
- Adds a new app interface method `AppRequest::EnableAfterMemproofsProvided`, which allows enabling an app only if the app is in the `AppStatus::Disabled(DisabledAppReason::NotStartedAfterProvidingMemproofs)` state. Attempting to enable the app from other states (other than Running) will fail.
- Warrants are used under-the-hood in more places now:
  - When gossiping amongst authorities, if an authority has a warrant for some data being requested, they will send the warrant instead of the data to indicate the invalid status of that data
  - When requesting data through must\_get calls, warrants will be returned with the data. The data returned to the client remains the same, but under the hood any warrants will be cached for later use.
- Adds a `lineage` field to the DNA manifest, which declares forward compatibility for any hash in that list with this DNA
- Adds a `AdminRequest::GetCompatibleCells` method which returns CellId for all installed cells which use a DNA that is forward-compatible with a given DNA hash. This can be used to find a compatible cell for use with the `UseExisting` cell provisioning method (still to be implemented)

## 0.4.0-dev.11

## 0.4.0-dev.10

## 0.4.0-dev.9

- Warrants: When an authority rejects another agent’s authored data, that authority creates a Warrant which is gossiped to the offending agent’s Agent Activity Authority, who then serves that warrant along with any `get_agent_activity` request.
- The `warrants` field of `AgentActivity` is now populated with warrants for that agent.
- Authorities author ChainFork warrants when detecting two actions by the same author with the same `prev_action`

## 0.4.0-dev.8

## 0.4.0-dev.7

- App manifest now includes a new `membrane_proofs_deferred: bool` field, which allows the membrane proofs for the app’s cells to be provided at a time after installation, allowing the app’s UI to guide the process of creating membrane proofs.
- Adds new `AppStatus::AwaitingMemproofs` to indicate an app which was installed with `membrane_proofs_deferred`
- Adds new app websocket method `ProvideMemproofs` for use with `membrane_proofs_deferred`

## 0.4.0-dev.6

## 0.4.0-dev.5

- Moved the WASM cache from the data directory to a subdirectory of the data directory named `wasm-cache`. Old content won’t be removed and WASMs will have to be recompiled into the new cache. \#3920
- Remove deprecated functions `consistency_10s` and `consistency_60s`. Use `await_consistency` instead.
- Remove deprecated type `SweetEasyInline`. Use `SweetInlineZomes` instead.
- Remove deprecated methods `SweetInlineZomes::callback` and `SweetInlineZomes::integrity_callback`. Use `SweetInlineZomes::function` and `SweetInlineZomes::integrity_function` instead.

## 0.4.0-dev.4

- Rename feature `sweetest` in Holochain crate to `sweettest` to match the crate name.
- App validation workflow: Reduce interval to re-trigger when dependencies are missing from 10 seconds to 100-1000 ms, according to number of missing dependencies.

## 0.4.0-dev.3

- App validation workflow: Fix bug where ops were stuck in app validation when multiple ops were requiring the same action or entry hash. Such ops were erroneously filtered out from validation for being marked as ops awaiting hashes and not unmarked as awaiting once the hashes had arrived.

## 0.4.0-dev.2

- System validation: Added a new rule that no new actions are allowed following a chain close action.
- App validation workflow: Add module-level documentation.
- Validation: Remove unused type `DhtOpOrder`. This type is superseded by `OpOrder`.

## 0.4.0-dev.1

- **BREAKING** - Serialization: Update of serialization packages `holochain-serialization` and `holochain-wasmer-*` leads to general message format change for enums. Previously an enum value like

<!-- end list -->

``` rust
enum Enum {
  Variant1,
  Variant2,
}
let value = Enum::Variant1;
```

was serialized as (JSON representation)

``` json
{
  "value": {
    "variant1": null
  }
}
```

Now it serializes to

``` json
{
  "value": "variant1"
}
```

- Adds a new admin interface call `RevokeAppAuthenticationToken` to revoke issued app authentication tokens. \#3765
- App validation workflow: Validate ops in sequence instead of in parallel. Ops validated one after the other have a higher chance of being validated if they depend on earlier ops. When validated in parallel, they potentially needed to await a next workflow run when the dependent op would have been validated.

## 0.4.0-dev.0

## 0.3.0

## 0.3.0-beta-dev.48

## 0.3.0-beta-dev.47

- Connections to Holochain app interfaces are now app specific, so anywhere that you used to have to provide an `installed_app_id` or `app_id` in requests, that is no longer required and has been removed. For example, `AppRequest::AppInfo` no longer takes any parameters and will return information about the app the connection is authenticated with. \#3643
- Signals are now only sent to clients that are connected to the app emitting the signal. When a cell is created by the conductor, it gets the ability to broadcast signals to any clients that are connected to the app that the cell is part of. When a client authenticates a connection to an app interface, the broadcaster for that app is found and attached to the connection. Previously all connected clients saw all signals, and there was no requirement to authenticate before receiving them. This is important to be aware of - if you connect to an app interface for signals only, you will still have to authenticate before receiving signals. \#3643
- App websocket connections now require authentication. There is a new admin operation `AdminRequest::IssueAppAuthenticationToken` which must be used to issue a connection token for a specific app. That token can be used with any app interface that will permit a connection to that app. After establishing a client connection, the first message must be an Authenticate message (rather than Request or Signal) and contain an `AppAuthenticationRequest` as its payload. \#3622
- When creating an app interface with `AdminRequest::AttachAppInterface` it is possible to specify an `installed_app_id` which will require that connections to that app interface are for the specified app. \#3622
- `AdminRequest::ListAppInterfaces` has been changed from returning a list of ports to return a list of `AppInterfaceInfo` which includes the port as well as the `installed_app_id` and `allowed_origins` for that interface. \#3622

## 0.3.0-beta-dev.46

## 0.3.0-beta-dev.45

- App validation workflow: Mock network in unit tests using new type `GenericNetwork` to properly test `must_get_agent_activity`. Previously that was not possible, as all peers in a test case were authorities for each other and `must_get_agent_activity` would therefore not send requests to the network.
- App validation workflow: Skip ops that have missing dependencies. If an op is awaiting dependencies to be fetched, it will be excluded from app validation.
- App validation workflow: Integration workflow is only triggered when some ops have been validated (either accepted or rejected).
- App validation workflow: While op dependencies are missing and being fetched, the workflow is re-triggering itself periodically. It’ll terminate this re-triggering after an interval in which no more missing dependencies could be fetched.

## 0.3.0-beta-dev.44

- App validation workflow: Refactored to not wait for ops that the op being validated depends on, that are being fetched and thus keep the workflow occupied. The workflow no longer awaits the dependencies and instead sends off fetch requests in the background.
- `consistency_10s` and `consistency_60s` from `holochain::sweettest` are deprecated. Use `await_consistency` instead.

## 0.3.0-beta-dev.43

- BREAKING: Holochain websockets now require an `allowed_origins` configuration to be provided. When connecting to the websocket a matching origin must be specified in the connection request `Origin` header. [\#3460](https://github.com/holochain/holochain/pull/3460)
  - The `ConductorConfiguration` has been changed so that specifying an admin interface requires an `allowed_origins` as well as the port it already required.
  - `AdminRequest::AddAdminInterfaces` has been updated as per the previous point.
  - `AdminRequest::AttachAppInterface` has also been updated so that attaching app ports requires an `allowed_origins` as well as the port it already required.
- BREAKING: Split the authored database by author. It was previous partitioned by DNA only and each agent that shared a DB because they were running the same DNA would have to share the write lock. This is a pretty serious bottleneck when the same app is being run for multiple agents on the same conductor. They are now separate files on disk and writes can proceed independently. There is no migration path for this change, if you have existing databases they will not be found. [\#3450](https://github.com/holochain/holochain/pull/3450)

## 0.3.0-beta-dev.42

## 0.3.0-beta-dev.41

## 0.3.0-beta-dev.40

## 0.3.0-beta-dev.39

## 0.3.0-beta-dev.38

- Some of the function signatures around SweetConductor app installation have changed slightly. You may need to use a slice (`&[x]`) instead of a collection of references (`[&x]`), or vice versa, in some places. If this is cumbersome please open an issue. [\#3310](https://github.com/holochain/holochain/pull/3310)
- Start refactoring app validation workflow by simplifying main validation loop. All op validations are awaited at once now instead of creating a stream of tasks and processing it in the background.

## 0.3.0-beta-dev.37

## 0.3.0-beta-dev.36

- Added `lair_keystore_version_req` to the output of `--build-info` for Holochain.
- BREAKING: Changed `post_commit` behavior so that it only gets called after a commit to the source chain. Previously, it would get called after every zome call, regardless of if a commit happened. [\#3302](https://github.com/holochain/holochain/pull/3302)
- Fixed a performance bug: various extra tasks were being triggered after every zome call which are only necessary if the zome call resulted in commits to the source chain. The fix should improve performance for read-only zome calls. [\#3302](https://github.com/holochain/holochain/pull/3302)
- Fixed a bug during the admin call `GrantZomeCallCapability`, where if the source chain had not yet been initialized, it was possible to create a capability grant before the `init()` callback runs. Now, `init()` is guaranteed to run before any cap grants are created.
- Updates sys validation to allow the timestamps of two actions on the same chain to be equal, rather than requiring them to strictly increasing.

## 0.3.0-beta-dev.35

- There is no longer a notion of “joining the network”. Previously, apps could fail to be enabled, accompanied by an error “Timed out trying to join the network” or “Error while trying to join the network”. Now, apps never fail to start for this reason. If the network cannot be reached, the app starts anyway. It is up to the UI to determine whether the node is in an “online” state via `AppRequest::NetworkInfo` (soon-to-be improved with richer information).
- CellStatus is deprecated and only remains in areas where deserialization would break if it were removed. The only valid CellStatus now is `CellStatus::Joined`.

## 0.3.0-beta-dev.34

- Fix: Wasmer cache was deserializing modules for every zome call which slowed them down. Additionally the instance cache that was supposed to store callable instances of modules was not doing that correctly. A cache for deserialized modules has been re-introduced and the instance cache was removed, following recommendation from the wasmer team regarding caching.
- Fix: Call contexts of internal callbacks like `validate` were not cleaned up from an in-memory map. Now external as well as internal callbacks remove the call contexts from memory. This is covered by a test.
- **BREAKING CHANGE:** Wasmer-related items from `holochain_types` have been moved to crate `holochain_wasmer_host::module`.
- Refactor: Every ribosome used to create a separate wasmer module cache. During app installation of multiple agents on the same conductor, the caches weren’t used, regardless of whether that DNA is already registered or not. The module cache is now moved to the conductor and kept there as a single instance.

## 0.3.0-beta-dev.33

- Make sqlite-encrypted a default feature

- Sys validation will no longer check the integrity with the previous action for StoreRecord or StoreEntry ops. These ‘store record’ checks are now only done for RegisterAgentActivity ops which we are sent when we are responsible for validating an agents whole chain. This avoids fetching and caching ops that we don’t actually need.

## 0.3.0-beta-dev.32

## 0.3.0-beta-dev.31

## 0.3.0-beta-dev.30

## 0.3.0-beta-dev.29

- Sys validation will now validate that a DeleteLink points to an action which is a CreateLink through the `link_add_address` of the delete.

## 0.3.0-beta-dev.28

- Fix an issue where app validation for StoreRecord ops with a Delete or DeleteLink action were always passed to all zomes. These ops are now only passed to the zome which defined the entry type of the op that is being deleted. [\#3107](https://github.com/holochain/holochain/pull/3107)
- Wasmer bumped from 4.2.2 to 4.2.4 [\#3025](https://github.com/holochain/holochain/pull/3025)
- Compiled wasms are now persisted to the file system so no longer need to be recompiled on subsequent loads [\#3025](https://github.com/holochain/holochain/pull/3025)
- **BREAKING CHANGE** Several changes to the file system [\#3025](https://github.com/holochain/holochain/pull/3025):
  - The environment path in config file is now called `data_root_path`
  - The `data_root_path` is no longer optional so MUST be specified in config
  - Interactive mode is no longer supported, so paths MUST be provided in config
  - The database is in a `databases` subdirectory of the `data_root_path`
  - The keystore now consistently uses a `ks` directory, was previously inconsistent between `ks` and `keystore`
  - The compiled wasm cache now exists and puts artifacts in the `wasm` subdirectory of the `data_root_path`

## 0.3.0-beta-dev.27

- Refactor: Remove shadowing glob re-exports that were shadowing other exports.

- Fix: Countersigning test `lock_chain` which ensures that source chain is locked while in a countersigning session.

- Major refactor of the sys validation workflow to improve reliability and performance:
  
  - Reliability: The workflow will now prioritise validating ops that have their dependencies available locally. As soon as it has finished with those it will trigger app validation before dealing with missing dependencies.
  - Reliability: For ops which have dependencies we aren’t holding locally, the network get will now be retried. This was a cause of undesirable behaviour for validation where a failed get would result in validation for ops with missing dependencies not being retried until new ops arrived. The workflow now retries the get on an interval until it finds dependencies and can proceed with validation.
  - Performance and correctness: A feature which captured and processed ops that were discovered during validation has been removed. This had been added as an attempt to avoid deadlocks within validation but if that happens there’s a bug somewhere else. Sys validation needs to trust that Holochain will correctly manage its current arc and that we will get that data eventually through publishing or gossip. This probably wasn’t doing a lot of harm but it was uneccessary and doing database queries so it should be good to have that gone.
  - Performance: In-memory caching for sys validation dependencies. When we have to wait to validate an op because it has a missing dependency, any other actions required by that op will be held in memory rather than being refetched from the database. This has a fairly small memory footprint because actions are relatively small but saves repeatedly hitting the cascade for the same data if it takes a bit of time to find a dependency on the network.

- **BREAKING* CHANGE*: The `ConductorConfig` has been updated to add a new option for configuring conductor behaviour. This should be compatible with existing conductor config YAML files but if you are creating the struct directly then you will need to include the new field. Currently this just has one setting which controls how fast the sys validation workflow will retry network gets for missing dependencies. It’s likely this option will change in the near future.

## 0.3.0-beta-dev.26

## 0.3.0-beta-dev.25

- Fix: In many cases app validation would not be retriggered for ops that failed validation. Previously the app validation workflow had been retriggered only when the number of concurrent ops to be validated (50) was reached. Now the workflow will be retriggered whenever any ops could not be validated.

- Added a new check to system validation to ensure that the `original_entry_address` of an update points to the same entry hash that the original action pointed to. [3023](https://github.com/holochain/holochain/pull/3023)

## 0.3.0-beta-dev.24

## 0.3.0-beta-dev.23

## 0.3.0-beta-dev.22

- Fix an issue where enough validation receipts being received would not prevent the publish workflow from continuing to run. This was a terrible waste of data and compute and would build up over time as Holochain is used. [2931](https://github.com/holochain/holochain/pull/2931)
- Improve log output for op publishing to accurately reflect the number of ops to be published. The number published which is logged later is accurate and it was confusing to see more ops published than were supposed to be. [2922](https://github.com/holochain/holochain/pull/2922)
- Fix an issue which prevented the publish loop for a cell from suspending if there was either 1. publish activity pending for other cells or 2. enough validation receipts received. [2922](https://github.com/holochain/holochain/pull/2922)

## 0.3.0-beta-dev.21

- Fix an issue where receiving incoming ops can accidentally filter out some DHT data until Holochain is restarted. The state management for in-flight DHT ops is now guaranteed by a `Drop` implementation which will clean up state when the `incoming_dht_ops_workflow` finishes. [2913](https://github.com/holochain/holochain/pull/2913)
- Performance improvement when sending validation receipts. When a batch of DHT ops is being processed and an author is unreachable it will no longer spend time trying to send more receipts to that author in serial and instead it sends receipts as a single batch per author. [2848](https://github.com/holochain/holochain/pull/2848)
- Resilience improvement with handling keystore errors in the validation receipt workflow. Previously, all errors caused the workflow to restart from the beginning. This was good for transient errors such as the keystore being unavailable but it also meant that a single validation receipt failing to be signed (e.g. due to a local agent key being removed from the keystore) would prevent any more validation receipts being sent by that conductor. [2848](https://github.com/holochain/holochain/pull/2848)
- **BREAKING CHANGE** Addressed an outstanding technical debt item to make the validation receipt workflow send a network notification (fire and forget) rather than waiting for a response. When the validation receipt workflow was written this functionality wasn’t available but now that it is, sending validation receipts can be sped up by not waiting for a peer to respond. The format has also been changed from sending one receipt at a time to sending batches so it was not possible to maintain backwards compatibility here. [2848](https://github.com/holochain/holochain/pull/2848)

## 0.3.0-beta-dev.20

## 0.3.0-beta-dev.19

- Fix: App interfaces are persisted when shutting down conductor. After restart, app interfaces without connected receiver websocket had signal emission fail altogether. Send errors are only logged now instead.

## 0.3.0-beta-dev.18

## 0.3.0-beta-dev.17

- Change `GenesisFailed` error to include `CellId` so that genesis failures can be correlated with the cells that failed. [2616](https://github.com/holochain/holochain/pull/2616)

## 0.3.0-beta-dev.16

## 0.3.0-beta-dev.15

- **BREAKING CHANGE** updating the project lock file to use the latest version of `serde` at `1.0.185` has changed how enums get serialized and as a knock on effect it has changed some hashes. This will make databases from previous versions incompatible with the next version of Holochain.

## 0.3.0-beta-dev.14

## 0.3.0-beta-dev.13

## 0.3.0-beta-dev.12

## 0.3.0-beta-dev.11

- Improves error messages when validation fails with an InvalidCommit error
- Fixed bug where if signature verification fails due to the lair service being unavailable, validation could fail. Now, that failure is treated as a normal error, so validation cannot proceed. [\#2604](https://github.com/holochain/holochain/pull/2604)

## 0.3.0-beta-dev.10

- Adds experimental Chain Head Coordinator feature, allowing multiple machines to share the same source chain. Holochain must be built with the `chc` feature flag (disabled by default).

## 0.3.0-beta-dev.9

## 0.3.0-beta-dev.8

## 0.3.0-beta-dev.7

- Fixes race condition which caused network instability. Newly joined nodes can get temporarily blocked by other nodes, causing connections to be repeatedly dropped. [\#2534](https://github.com/holochain/holochain/pull/2534)

## 0.3.0-beta-dev.6

## 0.3.0-beta-dev.5

- **BREAKING CHANGE**: The DhtOp validation rules have been significantly expanded upon, and some logic around what ops are produced when has been altered. Your existing app may experience rejected ops due to these more strict rules.

## 0.3.0-beta-dev.4

## 0.3.0-beta-dev.3

## 0.3.0-beta-dev.2

## 0.3.0-beta-dev.1

## 0.3.0-beta-dev.0

- The feature `test_utils` is no longer a default feature. To consume `sweetest` from this crate please now use `default-features = false` and the feature `sweetest`.

## 0.2.0

## 0.2.0-beta-rc.7

## 0.2.0-beta-rc.6

- Feature renaming from `no-deps` to `sqlite` and `db-encryption` to `sqlite-encrypted`. It should not be necessary to configure these unless you are packaging `holochain` or have imported it as a dependency without default features. In the latter case, please update any references to the old feature names.

## 0.2.0-beta-rc.5

- Implements the `clone_only` cell provisioning strategy, desgined for situations where no cell should be installed upon app installation but clones may be created later, via `roles[].provisioning.strategy` in the app manifest [\#2243](https://github.com/holochain/holochain/pull/2243)

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

- BREAKING CHANGE - Removes conductor networking types “Proxy” (“proxy”) and “Quic” (“quic”). Please transition to “WebRTC” (“webrtc”). [\#2208](https://github.com/holochain/holochain/pull/2208)
- Adds `DumpNetworkStats` api to admin websocket [\#2182](https://github.com/holochain/holochain/pull/2182).
- System validation now ensures that all records in a source chain are by the same author [\#2189](https://github.com/holochain/holochain/pull/2189)

## 0.2.0-beta-rc.2

- Fixes bug where supplying a `network_seed` during an `InstallApp` call does not actually update the network seed for roles whose `provisioning` is set to `None` in the manifest. Now the network seed is correctly updated. [\#2102](https://github.com/holochain/holochain/pull/2102)

- If AppManifest specifies an `installed_hash` for a DNA, it will check the conductor for an already-registered DNA at that hash, ignoring the DNA passed in as part of the bundle. Note that this means you can install apps without passing in any DNA, if the DNAs are already installed in the conductor. [\#2157](https://github.com/holochain/holochain/pull/2157)

- Adds new functionality to the conductor admin API which returns disk storage information. The storage used by apps is broken down into blobs which are being used by one or more app.

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

- When uninstalling an app, local data is now cleaned up where appropriate. [\#1805](https://github.com/holochain/holochain/pull/1805)
  - Detail: any time an app is uninstalled, if the removal of that app’s cells would cause there to be no cell installed which uses a given DNA, the databases for that DNA space are deleted. So, if you have an app installed twice under two different agents and uninstall one of them, no data will be removed, but if you uninstall both, then all local data will be cleaned up. If any of your data was gossiped to other peers though, it will live on in the DHT, and even be gossiped back to you if you reinstall that same app with a new agent.
- Renames `OpType` to `FlatOp`, and `Op::to_type()` to `Op::flattened()`. Aliases for the old names still exist, so this is not a breaking change. [\#1909](https://github.com/holochain/holochain/pull/1909)
- Fixed a [problem with validation of Ops with private entry data](https://github.com/holochain/holochain/issues/1861), where  `Op::to_type()` would fail for private `StoreEntry` ops. [\#1910](https://github.com/holochain/holochain/pull/1910)

## 0.1.0

## 0.1.0-beta-rc.4

- Fix: Disabled clone cells are no longer started when conductor restarts. [\#1775](https://github.com/holochain/holochain/pull/1775)

## 0.1.0-beta-rc.3

- Fix: calling `emit_signal` from the `post_commit` callback caused a panic, this is now fixed [\#1749](https://github.com/holochain/holochain/pull/1749)
- Fix: When you install an app with a cell that already exists for the same agent, the installation will error now. [\#1773](https://github.com/holochain/holochain/pull/1773)
- Fixes problem where disabling and re-enabling an app causes all of its cells to become unresponsive to any `get*` requests. [\#1744](https://github.com/holochain/holochain/pull/1744)
- Fixes problem where a disabled cell can continue to respond to zome calls and transmit data until the conductor is restarted. [\#1761](https://github.com/holochain/holochain/pull/1761)
- Adds Ctrl+C handling, so that graceful conductor shutdown is possible. [\#1761](https://github.com/holochain/holochain/pull/1761)
- BREAKING CHANGE - Added zome name to the signal emitted when using `emit_signal`.

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

- All zome calls must now be signed by the provenance, the signature is of the hash of the unsigned zome call, a unique nonce and expiry is also required [1510](https://github.com/holochain/holochain/pull/1510/files)

## 0.0.175

- BREAKING CHANGE - `ZomeId` and `zome_id` renamed to `ZomeIndex` and `zome_index` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppEntryType.id` renamed to `AppEntryType.entry_index` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppEntryType` renamed to `AppEntryDef` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppEntryDefName` renamed to `AppEntryName` [\#1667](https://github.com/holochain/holochain/pull/1667)
- BREAKING CHANGE - `AppRoleId` renamed to `RoleName` [\#1667](https://github.com/holochain/holochain/pull/1667)

## 0.0.174

- BREAKING CHANGE - The max entry size has been lowered to 4MB (strictly 4,000,000 bytes) [\#1659](https://github.com/holochain/holochain/pull/1659)
- BREAKING CHANGE - `emit_signal` permissions are changed so that it can be called during `post_commit`, which previously was not allowed [\#1661](https://github.com/holochain/holochain/pull/1661)

## 0.0.173

## 0.0.172

- BREAKING CHANGE - Update wasmer crate dependency [\#1620](https://github.com/holochain/holochain/pull/1620)
- Adds GossipInfo app interface method, which returns data about historical gossip progress which can be used to implement a progress bar in app UIs. [\#1649](https://github.com/holochain/holochain/pull/1649)
- BREAKING CHANGE - Add `quantum_time` as a DNA modifier. The default is set to 5 minutes, which is what it was previously hardcoded to. DNA manifests do not need to be updated, but this will change the DNA hash of all existing DNAs.

## 0.0.171

## 0.0.170

- Add call to authorize a zome call signing key to Admin API [\#1641](https://github.com/holochain/holochain/pull/1641)
- Add call to request DNA definition to Admin API [\#1641](https://github.com/holochain/holochain/pull/1641)

## 0.0.169

## 0.0.168

- Fixes bug that causes crash when starting a conductor with a clone cell installed

## 0.0.167

- Adds `SweetConductorConfig`, which adds a few builder methods for constructing variations of the standard ConductorConfig

## 0.0.166

- Fix restore clone cell by cell id. This used to fail with a “CloneCellNotFound” error. [\#1603](https://github.com/holochain/holochain/pull/1603)

## 0.0.165

- Revert requiring DNA modifiers when registering a DNA. These modifiers were optional before and were made mandatory by accident.

## 0.0.164

- Add App API call to archive an existing clone cell. [\#1578](https://github.com/holochain/holochain/pull/1578)
- Add Admin API call to restore an archived clone cell. [\#1578](https://github.com/holochain/holochain/pull/1578)
- Add Admin API call to delete all archived clone cells of an app’s role. For example, there is a base cell with role `document` and clones `document.0`, `document.1` etc.; this call deletes all clones permanently that have been archived before. This is not reversable; clones cannot be restored afterwards. [\#1578](https://github.com/holochain/holochain/pull/1578)

## 0.0.163

- Fixed rare “arc is not quantizable” panic, issuing a warning instead. [\#1577](https://github.com/holochain/holochain/pull/1577)

## 0.0.162

- **BREAKING CHANGE**: Implement App API call `CreateCloneCell`. **Role ids must not contain a dot `.` any more.** Clone ids make use of the dot as a delimiter to separate role id and clone index. [\#1547](https://github.com/holochain/holochain/pull/1547)
- Remove conductor config legacy keystore config options. These config options have been broken since we removed legacy lair in \#1518, hence this fix itself is not a breaking change. Also adds the `lair_server_in_proc` keystore config option as the new default to run an embedded lair server inside the conductor process, no longer requiring a separate system process. [\#1571](https://github.com/holochain/holochain/pull/1571)

## 0.0.161

## 0.0.160

## 0.0.159

- Updates TLS certificate handling so that multiple conductors can share the same lair, but use different TLS certificates by storing a “tag” in the conductor state database. This should not be a breaking change, but *will* result in a new TLS certificate being used per conductor. [\#1519](https://github.com/holochain/holochain/pull/1519)

## 0.0.158

## 0.0.157

## 0.0.156

- Effectively disable Wasm metering by setting the cranelift cost\_function to always return 0. This is meant as a temporary stop-gap and give us time to figure out a configurable approach. [\#1535](https://github.com/holochain/holochain/pull/1535)

## 0.0.155

- **BREAKING CHANGE** - Removes legacy lair. You must now use lair-keystore \>= 0.2.0 with holochain. It is recommended to abandon your previous holochain agents, as there is not a straight forward migration path. To migrate: [dump the old keys](https://github.com/holochain/lair/blob/v0.0.11/crates/lair_keystore/src/bin/lair-keystore/main.rs#L38) -\> [write a utility to re-encode them](https://github.com/holochain/lair/tree/hc_seed_bundle-v0.1.2/crates/hc_seed_bundle) -\> [then import them to the new lair](https://github.com/holochain/lair/tree/lair_keystore-v0.2.0/crates/lair_keystore#lair-keystore-import-seed---help) – [\#1518](https://github.com/holochain/holochain/pull/1518)
- New solution for adding `hdi_version_req` field to the output of `--build-info` argument. [\#1523](https://github.com/holochain/holochain/pull/1523)

## 0.0.154

- Revert: “Add the `hdi_version_req` key:value field to the output of the `--build-info` argument” because it broke. [\#1521](https://github.com/holochain/holochain/pull/1521)
  
  Reason: it causes a build failure of the *holochain*  crate on crates.io

## 0.0.153

- Add the `hdi_version_req` key:value field to the output of the `--build-info` argument

## 0.0.152

- Adds `AdminRequest::UpdateCoordinators` that allows swapping coordinator zomes for a running happ.

## 0.0.151

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)
- Allow deterministic bindings (dna\_info() & zome\_info()) to the genesis self check [\#1491](https://github.com/holochain/holochain/pull/1491).

## 0.0.150

## 0.0.149

## 0.0.148

- Added networking logic for enzymatic countersigning [\#1472](https://github.com/holochain/holochain/pull/1472)
- Countersigning authority response network message changed to a session negotiation enum [/\#1472](https://github.com/holochain/holochain/pull/1472)

## 0.0.147

## 0.0.146

## 0.0.145

**MAJOR BREAKING CHANGE\!** This release includes a rename of two Holochain core concepts, which results in a LOT of changes to public APIs and type names:

- “Element” has been renamed to “Record”
- “Header” has been renamed to “Action”

All names which include these words have also been renamed accordingly.

As Holochain has evolved, the meaning behind these concepts, as well as our understanding of them, has evolved as well, to the point that the original names are no longer adequate descriptors. We chose new names to help better reflect what these concepts mean, to bring more clarity to how we write and talk about Holochain.

## 0.0.144

- Add functional stub for `x_salsa20_poly1305_shared_secret_create_random` [\#1410](https://github.com/holochain/holochain/pull/1410)
- Add functional stub for `x_salsa20_poly1305_shared_secret_export` [\#1410](https://github.com/holochain/holochain/pull/1410)
- Add functional stub for `x_salsa20_poly1305_shared_secret_ingest` [\#1410](https://github.com/holochain/holochain/pull/1410)
- Limit conductor calls to `10_000_000_000` Wasm operations [\#1386](https://github.com/holochain/holochain/pull/1386)

## 0.0.143

## 0.0.142

## 0.0.141

## 0.0.140

## 0.0.139

- Udpate lair to 0.1.3 - largely just documentation updates, but also re-introduces some dependency pinning to fix mismatch client/server version check [\#1377](https://github.com/holochain/holochain/pull/1377)

## 0.0.138

## 0.0.137

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)
- Update legacy lair to 0.0.10 - allowing “panicky” flag [\#1349](https://github.com/holochain/holochain/pull/1349)
- Udpate lair to 0.1.1 - allowing usage in path with whitespace [\#1349](https://github.com/holochain/holochain/pull/1349)

## 0.0.136

## 0.0.135

## 0.0.134

## 0.0.133

## 0.0.132

## 0.0.131

- When joining the network set arc size to previous value if available instead of full to avoid network load [1287](https://github.com/holochain/holochain/pull/1287)

## 0.0.130

- Workflow errors generally now log rather than abort the current app [1279](https://github.com/holochain/holochain/pull/1279/files)

- Fixed broken links in Rust docs [\#1284](https://github.com/holochain/holochain/pull/1284)

## 0.0.129

## 0.0.128

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

## 0.0.127

- **BREAKING CHANGE** App validation callbacks are now run per `Op`. There is now only a single validation callback `fn validate(op: Op) -> ExternResult<ValidateCallbackResult>` that is called for each `Op`. See the documentation for `Op` for more details on what data is passed to the callback. There are example use cases in `crates/test_utils/wasm/wasm_workspace/`. For example in the `validate` test wasm. To update an existing app, you to this version all `validate_*` callbacks including `validate_create_link` must be changed to the new `validate(..)` callback. [\#1212](https://github.com/holochain/holochain/pull/1212).

- `RegisterAgentActivity` ops are now validated by app validation.

- Init functions can now make zome calls. [\#1186](https://github.com/holochain/holochain/pull/1186)

- Adds header hashing to `hash` host fn [1227](https://github.com/holochain/holochain/pull/1227)

- Adds blake2b hashing to `hash` host fn [1228](https://github.com/holochain/holochain/pull/1228)

## 0.0.126

## 0.0.125

## 0.0.124

## 0.0.123

- Fixes issue where holochain could get stuck in infinite loop when trying to send validation receipts. [\#1181](https://github.com/holochain/holochain/pull/1181).
- Additional networking metric collection and associated admin api `DumpNetworkMetrics { dna_hash: Option<DnaHash> }` for inspection of metrics [\#1160](https://github.com/holochain/holochain/pull/1160)
- **BREAKING CHANGE** - Schema change for metrics database. Holochain will persist historical metrics once per hour, if you do not clear the metrics database it will crash at that point. [\#1183](https://github.com/holochain/holochain/pull/1183)

## 0.0.122

- Adds better batching to validation workflows for much faster validation. [\#1167](https://github.com/holochain/holochain/pull/1167).

## 0.0.121

- **BREAKING CHANGE** Removed `app_info` from HDK [1108](https://github.com/holochain/holochain/pull/1108)
- Permissions on host functions now return an error instead of panicking [1141](https://github.com/holochain/holochain/pull/1141)
- Add `--build-info` CLI flag for displaying various information in JSON format. [\#1163](https://github.com/holochain/holochain/pull/1163)

## 0.0.120

## 0.0.119

## 0.0.118

- **BREAKING CHANGE** - Gossip now exchanges local peer info with `initiate` and `accept` request types. [\#1114](https://github.com/holochain/holochain/pull/1114).

## 0.0.117

## 0.0.116

## 0.0.115

- Fix [issue](https://github.com/holochain/holochain/issues/1100) where private dht ops were being leaked through the incoming ops sender. [1104](https://github.com/holochain/holochain/pull/1104).
- Kitsune now attempts to rebind the network interface in the event of endpoint shutdown. Note, it’s still recommended to bind to `0.0.0.0` as the OS provides additional resiliency for interfaces coming and going. [\#1083](https://github.com/holochain/holochain/pull/1083)
- **BREAKING CHANGE** current chain head including recent writes available in agent info [\#1079](https://github.com/holochain/holochain/pull/1079)
- **BREAKING (If using new lair)** If you are using the new (non-legacy) `lair_server` keystore, you will need to rebuild your keystore, we now pre-hash the passphrase used to access it to mitigate some information leakage. [\#1094](https://github.com/holochain/holochain/pull/1094)
- Better lair signature fallback child process management. The child process will now be properly restarted if it exits. (Note this can take a few millis on Windows, and may result in some signature errors.) [\#1094](https://github.com/holochain/holochain/pull/1094)

## 0.0.114

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

## 0.0.113

- Post commit is now infallible and expects no return value [PR1049](https://github.com/holochain/holochain/pull/1049)
- Always depend on `itertools` to make `cargo build --no-default-features` work [\#1060](https://github.com/holochain/holochain/pull/1060)
- `call_info` includes provenance and cap grant information [PR1063](https://github.com/holochain/holochain/pull/1063)
- Always depend on `itertools` to make `cargo build --no-default-features` work [\#1060](https://github.com/holochain/holochain/pull/1060)

## 0.0.112

- Always depend on `itertools` to make `cargo build --no-default-features` work [\#1060](https://github.com/holochain/holochain/pull/1060)

## 0.0.111

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

## 0.0.110

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

## 0.0.109

- Make validation run concurrently up to 50 DhtOps. This allows us to make progress on other ops when waiting for the network. [\#1005](https://github.com/holochain/holochain/pull/1005)
- FIX: Prevent the conductor from trying to join cells to the network that are already in the process of joining. [\#1006](https://github.com/holochain/holochain/pull/1006)

## 0.0.108

- Refactor conductor to use parking lot rw lock instead of tokio rw lock. (Faster and prevents deadlocks.). [\#979](https://github.com/holochain/holochain/pull/979).

### Changed

- The scheduler should work now

## 0.0.107

## 0.0.106

### Changed

- All Holochain `Timestamp`s (including those in Headers) are now at the precision of microseconds rather than nanoseconds. This saves 4 bytes per timestamp in memory and on disk.
- Various database field names changed. **Databases created in prior versions will be incompatible.**
- HDK `sys_time` now returns a `holochain_zome_types::prelude::Timestamp` instead of a `core::time::Duration`.
- Exposes `UninstallApp` in the conductor admin API.

## 0.0.105

## 0.0.104

- Updates lair to 0.0.4 which pins rcgen to 0.8.11 to work around [https://github.com/est31/rcgen/issues/63](https://github.com/est31/rcgen/issues/63)

## 0.0.103

### Fixed

- This release solves the issues with installing happ bundles or registering DNA via the admin API concurrently. [\#881](https://github.com/holochain/holochain/pull/881).

### Changed

- Header builder now uses chain top timestamp for new headers if in the future
- Timestamps in headers require strict inequality in sys validation

## 0.0.102

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

## 0.0.101

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

## 0.0.100

This is the first version number for the version of Holochain with a refactored state model (you may see references to it as Holochain RSM).

## 0.0.52-alpha2

*Note: Versions 0.0.52-alpha2 and older are belong to previous iterations of the Holochain architecture and are not tracked here.*
