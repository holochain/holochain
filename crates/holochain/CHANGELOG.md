---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- Adds DPKI support. This is not fully hooked up, so the main implication for this particular implementation is that you must be using the same DPKI implementation as all other nodes on the network that you wish to talk to. If the DPKI version mismatches, you cannot establish connections, and will see so as an error in the logs. This work is in preparation for future work which will make it possible to restore your keys if you lose your device, and to revoke and replace your keys if your device is stolen or compromised.

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
