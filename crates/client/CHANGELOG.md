---
default_semver_increment_mode: !pre_patch rc
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

### Added

- Added `CallZomeOptions` struct and `call_zome_with_options` / `signed_call_zome_with_options` methods to `AppWebsocket` to allow configuring a per-call timeout for zome calls. [\#5644](https://github.com/holochain/holochain/pull/5644)

## 0.8.1-rc.1

## 0.8.1-rc.0

## 0.8.0

## 0.8.0-rc.2

## 0.8.0-rc.1

## 0.8.0-rc.0

## 0.8.0-dev.30

## 0.8.0-dev.29

## 0.8.0-dev.28

## 0.8.0-dev.27

## 0.8.0-dev.26

## 0.8.0-dev.25

## 0.8.0-dev.24

## 0.8.0-dev.23

## 0.8.0-dev.22

## 0.8.0-dev.21

## 0.8.0-dev.20

## 0.8.0-dev.19

## 0.8.0-dev.18

## 0.8.0-dev.17

## 0.8.0-dev.16

## 0.8.0-dev.15

## 0.8.0-dev.14

## 0.8.0-dev.13

## 0.8.0-dev.12

## 0.8.0-dev.11

## 0.8.0-dev.10

## 0.8.0-dev.9

## 0.8.0-dev.8

- Add `agent_meta_info()` call to the `AdminWebsocket` and `AppWebsocket` to retrieve data from the peer meta store for a given agent by their Url [\#5043](https://github.com/holochain/holochain/pull/5043)

## 0.8.0-dev.7

## 0.8.0-dev.6

## 0.8.0-dev.5

## 0.8.0-dev.4

## 0.8.0-dev.3

## 0.8.0-dev.2

- Add `origin` argument to `AdminWebsocket::connect()` and `AppWebsocket::connect()`.

## 0.8.0-dev.1

## 0.8.0-dev.0

### Added

- `AppWebsocket::AgentInfo` call for apps to be able to list the discovered agents in their various DNAs.

### Changed

### Fixed

### Removed

## 0.7.0-rc.0

### Removed

- `AdminWebsocket::NetworkInfo` as the call was removed from the Conductor API. Use `DumpNetworkStats` and `DumpNetworkMetrics` instead, available on Admin and App websockets.
- `AdminWebsocket::GetCompatibleCells`, because the DNA lineage feature has been moved behind the `unstable-migration` feature in the Conductor API.

## 2025-02-27: 0.7.0-dev.3

### Added

- Re-export `CellInfo`, `ProvisionedCell`, `CellId`, `ClonedCell`, `ExternIO`, `GrantedFunctions`, `SerializedBytes` and `Timestamp` so that client users are less likely to need to import several Holochain libraries.
- Expose cached `AppInfo` from the `AppWebsocket` with a new `cached_app_info` method.
- Debug implementations for `AppWebsocket` and `AdminWebsocket`.
- New `connect_with_request_and_config` to expose the raw websocket connection parameters. This allows for more control over the connection setup, such as setting custom headers.
- The `connect_with_config` that already existed for the admin websocket now has an equivalent for the app websocket. This is useful if you want to change the timeout or other parameters of the websocket connection.
- A typedef `DynAgentSigner` for `Arc<dyn AgentSigner + Send + Sync>` which makes the type more convenient to use.
- Exported more common types so that uses are less likely to need to import other libraries, these are `AllowedOrigins` and `ConnectRequest`.

### Changed

- `connect*` methods on the `AppWebsocket` now return a `ConductorApiResult` instead of an `anyhow::Result`. This is consistent with the `AdminWebsocket`.
- It was possible to pass multiple socket addresses to the `connect` method of the `AppWebsocket` and `AdminWebsocket`. This allows you to try multiple addresses and connect to the first one that works. This wasn’t working because the client was just taking the first valid address and retrying connecting to that. Now the client will try each valid address, once, in turn.

### Removed

- Remove `again::retry` from client connect calls. It was preventing the client from trying all available addresses. If you need retry logic, please implement it in your application

## 2025-02-27: 0.7.0-dev.2

### Added

- Calls for several missing AdminWebsocket request types: `revoke_app_authentication_token`, `list_dnas`, `dump_state`, `dump_conductor_state`, `dump_full_state`, and `dump_network_metrics`.

### Changed

- Update to Holochain 0.5.0-dev.20.
- Make the `ConductorApiError` type implement error, for easier integration with other libraries.
- Several functions that used to return `anyhow::Result` now return a `ConductorApiResult`.
- The `on_signal` method of the `AppWebsocket` no longer returns an error, it cannot fail.

### Fixed

- The `AdminWebsocket` was not clone, which prevented using it across threads or async tasks.

## 2024-12-03: 0.7.0-dev.1

### Changed

- Update to Holochain 0.5.0-dev.7
- Updates to new zome call signing logic
- Uses the new `roles_settings` field in the `InstallAppPayload`.

## 2024-10-10: 0.7.0-dev.0

### Changed

- Update to Holochain 0.5.0-dev.0

## 2024-09-10: 0.6.0-dev.10

### Changed

- Update to Holochain 0.4.0-dev.27

## 2024-09-10: 0.6.0-dev.9

### Changed

- Update to Holochain 0.4.0-dev.25

## 2024-09-10: 0.6.0-dev.8

### Added

- Method to connect an admin websocket with a custom websocket configuration.

## 2024-08-31: 0.6.0-dev.7

### Added

- Admin calls `AgentInfo`, `AddAgentInfo` and `ListCellIds`.

## 2024-08-27: 0.6.0-dev.6

### Added

- Admin Websocket call `revoke_agent_key` which revokes an agent key for an app and makes the source chains of the app read-only.

## 2024-08-15: 0.6.0-dev.5

### Changed

- Listening for signals on the app websocket will now include system signals. These are only used for countersigning currently, so you can safely ignore them if you are not using countersigning.

## 2024-08-15: 0.6.0-dev.4

### Changed

- Uninstall app now has a `force` parameter. Please check Holochain documentation before setting this field to `true`\!

## 2024-07-16: 0.6.0-dev.3

### Added

- New value `NotStartedAfterProvidingMemproofs` for type `DisabledAppReason` which effectively allows a new app status, corresponding to the specific state where a UI has just called AppRequest::ProvideMemproofs, but the app has not yet been enabled for the first time.
- New `AppWebsocket` call `EnableAfterMemproofsProvided`, which allows enabling an app only if the app is in the `AppStatus::Disabled(DisabledAppReason::NotStartedAfterProvidingMemproofs)` state. Attempting to enable the app from other states (other than Running) will fail.
- New field `lineage` to the DNA manifest, which declares forward compatibility for any hash in that list with this DNA.
- New `AdminWebsocket` call `GetCompatibleCells`, which returns `CellId` for all installed cells which use a DNA that is forward-compatible with a given DNA hash. This can be used to find a compatible cell for use with the UseExisting cell provisioning method.

## 2024-07-04: 0.6.0-dev.2

### Changed

- The `ClientAgentSigner` function `add_credentials` no longer takes `self` as mutable. This wasn’t required by the function implementation.
- Updated to use Holochain 0.4.0-dev.11, which includes updates to several dependencies. Namely `holochain_serialized_bytes` has been updated to 0.0.55 which will force you to update your dependencies if you depend on other Holochain crates.

## 2024-06-10: 0.6.0-dev.1

### Added

- New call `AppRequest::ProvideMemproofs`. An app can be installed with deferred membrane proofs, which can later be provided through this call.

### Changed

- Remove unnecessary use of `&mut self` in the app and admin clients. These used to be needed when the internal `send` mutated state, but it no longer does that so we can drop the requirement for the clients to be mutable.

### Fixed

- Dropping admin or app connections will now close the connection.

## 2024-04-24: 0.5.0-dev.32

### Added

- New admin call `issue_app_auth_token` which allows you to issue an app auth token. This is now required when creating an app websocket connection. See the example for `AppWebsocket::connect` for how to use this.
- Missing app interface function `list_wasm_host_functions` has been added.

### Changed

- **BREAKING**: The admin call `list_app_interfaces` now returns a `Vec<AppInterfaceInfo>` instead of a `Vec<u16>`. You can map the response to a `Vec<u16>` to get the previous result.
- **BREAKING**: The admin call `attach_app_interface` now takes an additional parameter of type `InstalledAppId` which allows you to restrict an app interface to a single app.
- **BREAKING**: The app call `app_info` no longer takes an `installed_app_id` parameter. The conductor uses the app that you have authenticated with.

### Fixed

### Removed

- **BREAKING**: The old `AppWebsocket` is gone, its functionality was merged into the `AppAgentWebsocket` which has been renamed to `AppWebsocket`.

## 2024-03-27: 0.5.0-dev.31

### Changed

- **BREAKING**: The underlying package `holochain_websocket` changed. All websockets in this client follow the new `connect` function and take a socket address that implements `ToSocketAddr` instead of a URL `String`. Examples for the new parameter are `"localhost:30000"` and `([127.0.0.1], 30000)`. See trait [`ToSocketAddr`](https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html#tymethod.to_socket_addrs).
- **BREAKING**: The `attach_app_interface` method of the `AdminWebsocket` now takes an additional parameter of type `AllowedOrigins` which specifies what origins are allowed to connect to the created app interface.

## 2024-03-11: 0.5.0-dev.30

### Changed

- **BREAKING**: The underlying package `holochain_websocket` changed. All websockets in this client follow the new `connect` function and take a socket address that implements `ToSocketAddr` instead of a URL `String`. Examples for the new parameter are `"localhost:30000"` and `([127.0.0.1], 30000)`. See trait [`ToSocketAddr`](https://doc.rust-lang.org/std/net/trait.ToSocketAddrs.html#tymethod.to_socket_addrs).

## 2024-03-04: 0.5.0-dev.29

### Removed

- **BREAKING**: The utilities crate, it is now replaced by signing built into the client. Please see the updated tests for examples of how to use this.
- **BREAKING**: `sign_zome_call_with_client` which was used internally but also exposed in the public interface. You probably don’t need to call this but if you wish to for some reason then use one of the two new `*Signer` types, and convert them to a `Arc<Box<dyn AgentSigner>>`, then use the `sign` method to compute a signature. The logic to prepare the data to be signed is no longer public so you would have to set this up yourself following the `sign_zome_call` function in the `signer` module.

### Added

- Capability to create zome call signing credentials with the `AdminWebsocket` using `authorize_signing_credentials`.
- `ClientAgentSigner` type which can store (in memory) signing credentials created with `authorize_signing_credentials`.
- `LairAgentSigner` which is analagous to the `ClientAgentSigner` but is a wrapper around a Lair client instead so that private keys are stored in Lair.
- `from_existing` method to the `AppAgentWebsocket` which allows it to wrap an existing `AppWebsocket` instead of having to open a new connection. This is useful if you already have an `AppWebsocket` but otherwise you should just use the `connect` method of the `AppAgentWebsocket` rather than two steps.

### Changed

- **BREAKING**: `AppAgentWebsocket::connect` now takes an `Arc<Box<dyn AgentSigner>>` instead of a `LairClient`. The `Arc<Box<dyn AgentSigner>>` can be created from a `.into()` on either a `ClientAgentSigner` or a `LairAgentSigner`. Use the latter to restore the previous behaviour.
- **BREAKING**: `AppAgentWebsocket::call_zome` used to take a `RoleName` as its first parameter. This is now a `ZomeCallTarget`. There is a `.into()` which restores the previous behaviour. Now you can also pass a `CloneCellId` or a `CellId`, also using a `.into()`. Using `CellId` is stronly recommended for now. Please see the doc comments on `ZomeCallTarget` if you intend to use the other options.

## 2024-02-29: 0.5.0-dev.28

### Added

- Export `AdminWebsocket::EnableAppResponse` to be available downstream.

## 2024-02-01: 0.5.0-dev.27

### Added

- Added the `update_coordinators` call in the `AdminWebsocket`.

## 2024-01-26: 0.5.0-dev.26

### Added

- `AppAgentWebsocket` as an app websocket tied to a specific app and agent. Recommended for most applications.
- `on_signal`: event handler for reacting to app signals; implemented on `AppWebsocket` and `AppAgentWebsocket`.

### Changed

- Bump deps to holochain-0.3.0-beta-dev.26

## 2023-11-23: 0.5.0-dev.25

### Changed

- Bump deps to holochain-0.3.0-beta-dev.25

## 2023-11-15: 0.5.0-dev.24

### Changed

- Bump deps to holochain-0.3.0-beta-dev.24

## 2023-11-02: 0.5.0-dev.23

### Changed

- Bump deps to holochain-0.3.0-beta-dev.23

## 2023-10-20: 0.5.0-dev.0

### Changed

- Bump deps to holochain-0.3.0-beta-dev.22

## 2023-10-11: 0.4.5-rc.0

### Changed

- Remove unreachable code in `AppWebsocket::send`.
- Bump deps to holochain-0.2.3-beta-rc.0

### Fixed

- Upgrade to security patched version of `webpki`.

## 2023-10-02: 0.4.4

### Changed

- Pin serde to max v1.0.166 properly.

## 2023-09-28: 0.4.3

### Changed

- Pin serde to v1.0.166
- Upgrade holochain\_serialized\_bytes to 0.0.53

## 2023-09-13: 0.4.2

### Changed

- Upgrade to Holochain 0.2.2.

## 2023-09-11: 0.4.2-rc.3

### Changed

- Upgrade to Holochain 0.2.2-beta-rc.3.

## 2023-08-31: 0.4.2-rc.0

### Changed

- Upgrade to Holochain 0.2.2-beta-rc.0.

## 2023-08-07: 0.4.1

### Added

- Admin API call `graft_records`.

### Changed

- Upgrade to Holochain 0.2.1.

## 2023-04-21: 0.4.0

### Added

- Add `storage_info` to the admin websocket.
- Add `network_info` to the app websocket.

### Changed

- **BREAKING CHANGE**: Upgrade to Holochain 0.2 release candidate ahead of the holochain 0.2 release.

## 2023-02-15: 0.3.1

### Changed

- Upgrade to latest Holochain dependencies.
- Switch to Nix flake for develop environment. Run `nix develop` from now on instead of `nix-shell`. Pass on `--extra-experimental-features nix-command --extra-experimental-features flakes` or enable these features for your user in [`~/.config/nix/nix.conf`](https://nixos.org/manual/nix/stable/command-ref/conf-file.html#conf-experimental-features).

## 2023-01-23: 0.3.0

### Added

- Admin API call `get_dna_definition`
- Utility crate for authorizing credentials and signing zome calls

### Changed

- **BREAKING CHANGE**: Upgrade to Holochain 0.1.0-beta-rc.3
- **BREAKING CHANGE**: Require all zome calls to be signed.
- **BREAKING CHANGE**: Rename `install_app_bundle` to `install_app`.
- **BREAKING CHANGE**: Rename `archive_clone_cell` to `disable_clone_cell`.
- **BREAKING CHANGE**: Rename `restore_archived_clone_cell` to `enable_clone_cell`.
- **BREAKING CHANGE**: Move `enable_clone_cell` to App API.
- **BREAKING CHANGE**: Refactor `delete_clone_cell` to delete a single disabled clone cell.
- **BREAKING CHANGE**: Refactor `app_info` to return all cells and DNA modifiers.
- **BREAKING CHANGE**: Rename `request_agent_info` to `agent_info`.

## 2022-10-03: 0.2.0

Compatible with Holochain \>= 0.0.165

### Added

- Added calls for clone cell management:
  - App API: create clone cell
  - App API: archive clone cell
  - Admin API: restore clone cell
  - Admin API: delete archived clone cells
- Added test fixture and tests for clone cells calls

### Changed

- Upgrade to Holochain 0.0.165

## 2022-08-18: 0.1.1

### Changed

- Upgrade to Holochain 0.0.154

## 2022-01-20: 0.1.0

### Changed

- Upgrade to latest Holochain 0.0.147

## 2022-01-20: 0.0.1

### Added

- Initial release & publication as a crate
