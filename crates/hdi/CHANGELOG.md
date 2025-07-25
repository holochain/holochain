---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.7.0-dev.10

- **BREAKING CHANGE** [PR \#5129](https://github.com/holochain/holochain/pull/5129): Removed `hash` function from host functions. Use the `hash_action` and `hash_entry` functions from `HDI` instead.

## 0.7.0-dev.9

## 0.7.0-dev.8

## 0.7.0-dev.7

## 0.7.0-dev.6

## 0.7.0-dev.5

## 0.7.0-dev.4

## 0.7.0-dev.3

## 0.7.0-dev.2

## 0.7.0-dev.1

## 0.7.0-dev.0

## 0.6.0

## 0.6.0-rc.0

## 0.6.0-dev.16

## 0.6.0-dev.15

## 0.6.0-dev.14

- Prevent “TODO” comments from being rendered in cargo docs.

## 0.6.0-dev.13

## 0.6.0-dev.12

## 0.6.0-dev.11

- Update `holochain_wasmer_guest`

## 0.6.0-dev.10

- Update `holochain_wasmer_guest`, remove temporary fork of wasmer and update wasmer to 5.x.

## 0.6.0-dev.9

## 0.6.0-dev.8

## 0.6.0-dev.7

## 0.6.0-dev.6

## 0.6.0-dev.5

## 0.6.0-dev.4

## 0.6.0-dev.3

## 0.6.0-dev.2

## 0.6.0-dev.1

## 0.6.0-dev.0

## 0.5.0

## 0.5.0-dev.17

## 0.5.0-dev.16

## 0.5.0-dev.15

## 0.5.0-dev.14

## 0.5.0-dev.13

## 0.5.0-dev.12

## 0.5.0-dev.11

## 0.5.0-dev.10

## 0.5.0-dev.9

## 0.5.0-dev.8

## 0.5.0-dev.7

## 0.5.0-dev.6

## 0.5.0-dev.5

- Remove deprecated type `OpType`. Use `FlatOp` instead.
- Remove deprecated method `Op::to_type`. Use `Op::flattened` instead.

## 0.5.0-dev.4

## 0.5.0-dev.3

## 0.5.0-dev.2

## 0.5.0-dev.1

## 0.5.0-dev.0

## 0.4.0

## 0.4.0-beta-dev.36

- **BREAKING**: Original action and entry have been removed from relevant variants of `Op`. To use original action and entry during validation, they can be explicitly fetched with HDK calls `must_get_action` and `must_get_entry`. Op is passed into the app validation callback `validate` where validation rules of an app can be implemented. For update and delete operations original action and original entry used to be prefetched, regardless of whether they were used in `validate` or not. Particularly for an update or delete of an entry it is not common to employ the original entry in validation. It is therefore removed from those variants of `Op` which means a potential performance increase for not having to fetch original actions and entries for all ops to be validated.

## 0.4.0-beta-dev.35

## 0.4.0-beta-dev.34

## 0.4.0-beta-dev.33

## 0.4.0-beta-dev.32

## 0.4.0-beta-dev.31

## 0.4.0-beta-dev.30

## 0.4.0-beta-dev.29

## 0.4.0-beta-dev.28

## 0.4.0-beta-dev.27

## 0.4.0-beta-dev.26

## 0.4.0-beta-dev.25

## 0.4.0-beta-dev.24

## 0.4.0-beta-dev.23

## 0.4.0-beta-dev.22

## 0.4.0-beta-dev.21

## 0.4.0-beta-dev.20

## 0.4.0-beta-dev.19

## 0.4.0-beta-dev.18

## 0.4.0-beta-dev.17

- Migrate types for hash paths from hdk crate and include in prelude: Anchor, Path, Component, TypedPath [\#2980](https://github.com/holochain/holochain/pull/2980)

## 0.4.0-beta-dev.16

- Change the license from Apache-2.0 to CAL-1.0 to match the HDK.

## 0.4.0-beta-dev.15

## 0.4.0-beta-dev.14

## 0.4.0-beta-dev.13

## 0.4.0-beta-dev.12

## 0.4.0-beta-dev.11

## 0.4.0-beta-dev.10

## 0.4.0-beta-dev.9

## 0.4.0-beta-dev.8

## 0.4.0-beta-dev.7

## 0.4.0-beta-dev.6

## 0.4.0-beta-dev.5

## 0.4.0-beta-dev.4

## 0.4.0-beta-dev.3

## 0.4.0-beta-dev.2

## 0.4.0-beta-dev.1

## 0.4.0-beta-dev.0

## 0.3.0

## 0.3.0-beta-rc.5

## 0.3.0-beta-rc.4

## 0.3.0-beta-rc.3

## 0.3.0-beta-rc.2

## 0.3.0-beta-rc.1

## 0.3.0-beta-rc.0

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
