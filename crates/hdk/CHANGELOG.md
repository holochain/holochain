---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.4.0-dev.7

## 0.4.0-dev.6

## 0.4.0-dev.5

## 0.4.0-dev.4

## 0.4.0-dev.3

## 0.4.0-dev.2

- Adds two new HDK functions `close_chain` and `open_chain` that allow `Action::CloseChain` and `Action::OpenChain` respectively, to be created. These are intended to be used for DNA migrations. There is an example in the Holochain functions tests in ‘migration.rs’ \#3804

## 0.4.0-dev.1

## 0.4.0-dev.0

## 0.3.0

## 0.3.0-beta-dev.41

## 0.3.0-beta-dev.40

## 0.3.0-beta-dev.39

## 0.3.0-beta-dev.38

## 0.3.0-beta-dev.37

## 0.3.0-beta-dev.36

## 0.3.0-beta-dev.35

## 0.3.0-beta-dev.34

## 0.3.0-beta-dev.33

## 0.3.0-beta-dev.32

- Added `create_clone_cell`, `disable_clone_cell`, `enable_clone_cell` and `delete_clone_cell` functionality to the HDK. This was previously only available on the admin interface of Holochain which shouldn’t be used by apps. Exposing this functionality through the HDK allows happ developers to manage clones from their backend code without having to worry about their apps breaking when more security is added to the admin interface. The only restriction on the use of these methods is that they will not permit you to create clones in another app. You can create clones of any cell within the app you make the host function calls from.
- **BREAKING**: Added parameter `GetOptions` to calls `get_links` and `get_link_details`, to allow for fetching only local data. With the default setting of this option - `Latest`, links and link details are fetched from the network. When specifically set to `Content`, the network call is skipped and the calls only consider locally available data.

## 0.3.0-beta-dev.31

## 0.3.0-beta-dev.30

## 0.3.0-beta-dev.29

## 0.3.0-beta-dev.28

## 0.3.0-beta-dev.27

## 0.3.0-beta-dev.26

## 0.3.0-beta-dev.25

## 0.3.0-beta-dev.24

## 0.3.0-beta-dev.23

- **BREAKING CHANGE** Rename `remote_signal` to `send_remote_signal` to match the grammar of other HDK functions. [\#3113](https://github.com/holochain/holochain/pull/3113)

- Remove access to `Timestamp::now()` which comes from `kitsune_p2p_timestamp` and was not supposed to be available in WASM. It would always panic in WASM calls so it should be safe to assume that nobody was actually using this in real apps. If you were trying to and this breaks your hApp then please consider using `sys_time` from the HDK instead which is safe to use for getting the current time.

## 0.3.0-beta-dev.22

## 0.3.0-beta-dev.21

- Remove types for hash paths (migrated to hdi crate). Add HdkPathExt trait to implement TypedPath functionality that requires hdk. Add TryFromPath trait to implement conversion of Path into Anchor. [\#2980](https://github.com/holochain/holochain/pull/2980)

## 0.3.0-beta-dev.20

## 0.3.0-beta-dev.19

## 0.3.0-beta-dev.18

## 0.3.0-beta-dev.17

## 0.3.0-beta-dev.16

## 0.3.0-beta-dev.15

## 0.3.0-beta-dev.14

## 0.3.0-beta-dev.13

## 0.3.0-beta-dev.12

## 0.3.0-beta-dev.11

## 0.3.0-beta-dev.10

- **BREAKING CHANGE** `get_links` no longer takes `base`, `link_type` and `link_tag` as separate inputs and now takes `GetLinksInput` instead. This can be built using a `GetLinksInputBuilder`. Links can then be filtered by `author` and created timestamp `after` and `before`. This change has been made both to make the `get_links` function consistent with what you see if you use `HDK.with`, which is always supposed to be the case, and also to increase the options for filtering getting links.

## 0.3.0-beta-dev.9

## 0.3.0-beta-dev.8

## 0.3.0-beta-dev.7

- Add String<TryInto> for Path for easy conversion of Path to string representation

## 0.3.0-beta-dev.6

## 0.3.0-beta-dev.5

- New v2 of dna info returns full modifiers not just properties. Removed from genesis self check in favour of hdk call. [\#2366](https://github.com/holochain/holochain/pull/2366).

## 0.3.0-beta-dev.4

## 0.3.0-beta-dev.3

## 0.3.0-beta-dev.2

- Add new HDK function `count_links` which accepts a filter that can be applied remotely. This is a more optimal alternative to requesting all links and counting them within a zome function.

## 0.3.0-beta-dev.1

## 0.3.0-beta-dev.0

## 0.2.0

## 0.2.0-beta-rc.6

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

- Add block/unblock agent functions to HDK [\#1828](https://github.com/holochain/holochain/pull/1828)
- Rewrite hdk documentation and add links to conductor API docs.

## 0.1.0

- Add note in HDK documentation about links not deduplicating. ([\#1791](https://github.com/holochain/holochain/pull/1791))

## 0.1.0-beta-rc.3

- Fix typos and links in docs and add links to wasm examples.

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

## 0.0.163

## 0.0.162

## 0.0.161

## 0.0.160

## 0.0.159

## 0.0.158

## 0.0.157

- Pin the *hdi* dependency version. [\#1605](https://github.com/holochain/holochain/pull/1605)

## 0.0.156

## 0.0.155

## 0.0.154

## 0.0.153

## 0.0.152

## 0.0.151

## 0.0.150

## 0.0.149

## 0.0.148

## 0.0.147

## 0.0.146

## 0.0.145

## 0.0.144

- Docs: Add example how to get a typed path from a path to `path` module [\#1505](https://github.com/holochain/holochain/pull/1505)
- Exposed `TypedPath` type in the hdk prelude for easy access from zomes.

## 0.0.143

- Docs: Add documentation on `get_links` argument `link_type`. [\#1486](https://github.com/holochain/holochain/pull/1486)
- Docs: Intra-link to `wasm_error` and `WasmErrorInner`. [\#1486](https://github.com/holochain/holochain/pull/1486)

## 0.0.142

## 0.0.141

- Docs: Add section on coordinator zomes and link to HDI crate.

## 0.0.140

## 0.0.139

- **BREAKING CHANGE:** Anchor functions, `TypedPath` and `create_link` take `ScopedLinkType: TryFrom<T>` instead of `LinkType: From<T>`.
- **BREAKING CHANGE:** `create_entry` takes `ScopedEntryDefIndex: TryFrom<T>` instead of `EntryDefIndex: TryFrom<T>`.
- **BREAKING CHANGE:** `get_links` and `get_link_details` take `impl LinkTypeFilterExt` instead of `TryInto<LinkTypeRanges>`.
- hdk: **BREAKING CHANGE** `x_salsa20_poly1305_*` functions have been properly implemented. Any previous `KeyRef`s will no longer work. These new functions DO NOT work with legacy lair `v0.0.z`, you must use NEW lair `v0.y.z` (v0.2.0 as of this PR). [\#1446](https://github.com/holochain/holochain/pull/1446)
- Fixed `hdk::query`, which was showing some incorrect behavior [\#1402](https://github.com/holochain/holochain/pull/1402):
  - When using `ChainQueryFilterRange::ActionHashRange`, extraneous elements from other authors could be returned.
  - Certain combinations of filters, like hash-bounded ranges and header type filters, are currently implemented incorrectly and lead to undefined behavior. Filter combinations which are unsupported now result in `SourceChainError::UnsupportedQuery`.

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
