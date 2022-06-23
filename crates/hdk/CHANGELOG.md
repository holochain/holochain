---
unreleasable: false
default_unreleasable: true
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.0.138

- hdk: Bump rand version + fix getrandom (used by rand\_core and rand) to fetch randomness from host system when compiled to WebAssembly. [\#1445](https://github.com/holochain/holochain/pull/1445)

## 0.0.137

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

## 0.0.136

- Docs: Crate README generated from crate level doc comments [\#1392](https://github.com/holochain/holochain/pull/1392).

## 0.0.135

## 0.0.134

## 0.0.133

## 0.0.132

- hdk: Provide `Into<AnyLinkableHash>` impl for `EntryHash` and `HeaderHash`. This allows `create_link` and `get_links` to be used directly with EntryHash and HeaderHash arguments, rather than needing to construct an `AnyLinkableHash` explicitly.

## 0.0.131

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

## 0.0.130

## 0.0.129

## 0.0.128

*NOTE: this release has not been published to crates.io*

- hdk: Adds external hash type for data that has a DHT location but does not exist on the DHT [\#1298](https://github.com/holochain/holochain/pull/1298)
- hdk: Adds compound hash type for linkable hashes [\#1308](https://github.com/holochain/holochain/pull/1308)
- hdk: Missing dependencies are fetched async for validation [\#1268](https://github.com/holochain/holochain/pull/1268)

## 0.0.127

## 0.0.126

- Docs: Explain how hashes in Holochain are composed and its components on the module page for `hdk::hash` [\#1299](https://github.com/holochain/holochain/pull/1299).

## 0.0.125

- hdk: link base and target are no longer required to exist on the current DHT and aren’t made available via. validation ops (use must\_get\_entry instead) [\#1266](https://github.com/holochain/holochain/pull/1266)

## 0.0.124

## 0.0.123

## 0.0.122

- hdk: `delete`, `delete_entry`, and `delete_cap_grant` can all now take a `DeleteInput` as an argument to be able specify `ChainTopOrdering`, congruent with `create` and `update`. This change is backward compatible: a plain `HeaderHash` can still be used as input to `delete`.

## 0.0.121

## 0.0.120

- docs: Add introduction to front-page and move example section up [1172](https://github.com/holochain/holochain/pull/1172)

## 0.0.119

- hdk: `encoding` from `holo_hash` re-exported as hdk feature [1177](https://github.com/holochain/holochain/pull/1177)

## 0.0.118

- hdk: `Path` now split into `Path` and `PathEntry` [1156](https://github.com/holochain/holochain/pull/1156)
- hdk: Minor changes and additions to `Path` methods [1156](https://github.com/holochain/holochain/pull/1156)
- hdk: `call` and `call_remote` are the same thing under the hood [1180](https://github.com/holochain/holochain/pull/1180)

## 0.0.117

## 0.0.116

## 0.0.115

## 0.0.114

## 0.0.113

## 0.0.112

## 0.0.111

## 0.0.110

## 0.0.109

## 0.0.108

## 0.0.107

### Changed

- hdk: `scheduled` fn signature updated to a string

### Added

- hdk: `map_extern_infallible` added to map infallible externs
- hdk: `schedule` function now takes a String giving a function name to schedule, rather than a Duration

## 0.0.106

## 0.0.105

## 0.0.104

## 0.0.103

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

## 0.0.102

### Changed

- hdk: fixed wrong order of recipient and sender in `x_25519_x_salsa20_poly1305_decrypt`

## 0.0.101

### Changed

- Added `HdkT` trait to support mocking the host and native rust unit tests

### Added

- Added `sign_ephemeral` and `sign_ephemeral_raw`

## [0.0.100](https://github.com/holochain/holochain/compare/hdk-v0.0.100-alpha1..hdk-v0.0.100)

### Changed

- hdk: fixup the autogenerated hdk documentation.

## 0.0.100-alpha.1

### Added

- holochain 0.0.100 (RSM) compatibility
- Extensive doc comments
