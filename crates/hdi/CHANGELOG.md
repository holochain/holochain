---
default_semver_increment_mode: !pre_minor beta-rc
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.2.0

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

## 0.1.10

## 0.1.9

## 0.1.8

## 0.1.7

## 0.1.6

## 0.1.5

## 0.1.4

## 0.1.3

## 0.1.2

## 0.1.1

## 0.1.0

- Initial minor version bump. This indicates our impression that we have made significant progress towards stabilizing the detereministic integrity layer’s API. [\#1550](https://github.com/holochain/holochain/pull/1550)

## 0.0.21

## 0.0.20

- Adds `must_get_agent_activity` which allows depending on an agents source chain by using a deterministic hash bounded range query. [\#1502](https://github.com/holochain/holochain/pull/1502)

## 0.0.19

## 0.0.18

## 0.0.17

## 0.0.16

- Docs: Add `OpType` helper example to HDI validation section [\#1505](https://github.com/holochain/holochain/pull/1505)

## 0.0.15

- Adds the `OpHelper` trait to create the `OpType` convenience type to help with writing validation code. [\#1488](https://github.com/holochain/holochain/pull/1488)
- Docs: Add documentation on `LinkTypeFilterExt`. [\#1486](https://github.com/holochain/holochain/pull/1486)

## 0.0.14

- Docs: replace occurrences of `hdk_entry_def` and `entry_def!` with `hdk_entry_helper`.

## 0.0.13

- Docs: crate level documentation for `hdi`.

### Added

## 0.0.12

## 0.0.11

- `EntryTypesHelper`: `try_from_local_type` is removed and `try_from_global_type` becomes `deserialize_from_type`.
- `LinkTypesHelper` is removed.
- `LinkTypeFilterExt` is added to allow extra types to convert to `LinkTypeFilter`.

## 0.0.10

## 0.0.9

- Bump wasmer to 0.0.80 [\#1386](https://github.com/holochain/holochain/pull/1386)

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `EntryTypesHelper` helper trait for deserializing to the correct `Entry`.
- `LinkTypesHelper` helper trait for creating `LinkTypeRanges` that fit the current local scope.

### Removed

- `register_entry!` macro as it is no longer needed. Use `hdk_derive::hdk_entry_defs`.

## 0.0.8

## 0.0.7

- Fix broken wasm tracing. [PR](https://github.com/holochain/holochain/pull/1389).

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
