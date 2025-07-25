---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.6.0-dev.9

- **BREAKING**: Rename enum `ChainFilters` to `LimitConditions`
- **BREAKING**: Rename enum variant `LimitConditions::Until` to `LimitConditions:UntilHash`
- **BREAKING**: Add enum variant `UntilTimestamp` to `LimitConditions`
- **BREAKING**: Replace enum variant `LimitConditions::Both` with `LimitConditions::Multiple`

## 0.6.0-dev.8

- [Fixed issue 3606](https://github.com/holochain/holochain/issues/3606): Implemented `action_hash` for `Op`.

## 0.6.0-dev.7

## 0.6.0-dev.6

## 0.6.0-dev.5

## 0.6.0-dev.4

## 0.6.0-dev.3

## 0.6.0-dev.2

## 0.6.0-dev.1

## 0.6.0-dev.0

## 0.5.0

## 0.5.0-rc.0

## 0.5.0-dev.13

## 0.5.0-dev.12

- Prevent “TODO” comments from being rendered in cargo docs.

## 0.5.0-dev.11

## 0.5.0-dev.10

## 0.5.0-dev.9

## 0.5.0-dev.8

## 0.5.0-dev.7

## 0.5.0-dev.6

## 0.5.0-dev.5

## 0.5.0-dev.4

## 0.5.0-dev.3

## 0.5.0-dev.2

## 0.5.0-dev.1

## 0.5.0-dev.0

## 0.4.0

## 0.4.0-dev.15

## 0.4.0-dev.14

## 0.4.0-dev.13

## 0.4.0-dev.12

## 0.4.0-dev.11

## 0.4.0-dev.10

## 0.4.0-dev.9

## 0.4.0-dev.8

## 0.4.0-dev.7

## 0.4.0-dev.6

## 0.4.0-dev.5

## 0.4.0-dev.4

## 0.4.0-dev.3

## 0.4.0-dev.2

## 0.4.0-dev.1

## 0.4.0-dev.0

## 0.3.0

## 0.3.0-beta-dev.33

- **BREAKING**: Original action and entry have been removed from relevant variants of `Op`. To use original action and entry during validation, they can be explicitly fetched with HDK calls `must_get_action` and `must_get_entry`. Op is passed into the app validation callback `validate` where validation rules of an app can be implemented. For update and delete operations original action and original entry used to be prefetched, regardless of whether they were used in `validate` or not. Particularly for an update or delete of an entry it is not common to employ the original entry in validation. It is therefore removed from those variants of `Op` which means a potential performance increase for not having to fetch original actions and entries for all ops to be validated.

- Removed `DnaCompatParams` which was never fully hooked up and didn’t do anything.

## 0.3.0-beta-dev.32

## 0.3.0-beta-dev.31

## 0.3.0-beta-dev.30

## 0.3.0-beta-dev.29

## 0.3.0-beta-dev.28

## 0.3.0-beta-dev.27

## 0.3.0-beta-dev.26

## 0.3.0-beta-dev.25

## 0.3.0-beta-dev.24

## 0.3.0-beta-dev.23

- Adds `DnaCompatParams` to DnaDef, a new set of parameters that determines network compatibility between instances. These parameters are similar to DnaModifiers in that they affect the DNA hash, but they are not settable by the DNA dev – they are set automatically by the conductor at install time. This ensures that the same DNA installed into two different conductors with incompatible features will wind up with two different DNA hashes, so that they won’t attempt to communicate and fail.

## 0.3.0-beta-dev.22

## 0.3.0-beta-dev.21

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

## 0.3.0-beta-dev.9

## 0.3.0-beta-dev.8

## 0.3.0-beta-dev.7

## 0.3.0-beta-dev.6

## 0.3.0-beta-dev.5

## 0.3.0-beta-dev.4

## 0.3.0-beta-dev.3

## 0.3.0-beta-dev.2

## 0.3.0-beta-dev.1

## 0.3.0-beta-dev.0

## 0.2.0

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.3

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

- **BREAKING CHANGE**: Updated capability grant structure `GrantedFunctions` to be an enum with `All` for allowing all zomes all functions to be called, along with `Listed` to specify a zome and function as before. [\#1732](https://github.com/holochain/holochain/pull/1732)

## 0.1.0-beta-rc.0

## 0.0.25

## 0.0.24

## 0.0.23

## 0.0.22

## 0.0.21

## 0.0.20

## 0.0.19

## 0.0.18

## 0.0.17

## 0.0.16

- Adds `ChainFilter` type for use in `must_get_agent_activity`. This allows specifying a chain top hash to start from and then creates a range either to genesis or `unit` a given hash or after `take`ing a number of actions. The range iterates backwards from the given chain top till it reaches on of the above possible chain bottoms. For this reason it will never contain forks. [\#1502](https://github.com/holochain/holochain/pull/1502)

## 0.0.15

## 0.0.14

## 0.0.13

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## 0.0.12

## 0.0.11

## 0.0.10

- `ZomeId` added back to `CreateLink` and `AppEntryType`.
- `ScopedZomeTypesSet` has been simplified for easier use. Global and local types have been removed in favor of scoping `EntryDefIndex` and `LinkType` with the `ZomeId` of where they were defined.
- `LinkTypeRanges` has been removed.
- `LinkTypeFilter` replaces `LinkTypeRanges` as a more simplified way of filtering on `get_links`. `..` can be used to get all links from a zomes dependencies.
- `GlobalZomeTypeId` and `LocalZomeTypeId` removed.
- Links from integrity zomes that are not part of a coordinators dependency list are no longer accessible.
- In preparation for rate limiting, the inner Action structs which support app-defined “weights”, viz. `Create`, `Update`, `Delete`, and `CreateLink`, now have a `weight` field. This is currently set to a default value of “no weight”, but will later be used to store the app-defined weight.
  - A bit of deeper detail on this change: each of these action structs is now generic over the weight field, to allow “weighed” and “unweighed” versions of that header. This is necessary to be able to express these actions both before and after they have undergone the weighing process.

## 0.0.9

- Countersigning now accepts optional additional signers but the first must be the enzyme [\#1394](https://github.com/holochain/holochain/pull/1394)
- The first agent in countersigning is always the enzyme if enzymatic [\#1394](https://github.com/holochain/holochain/pull/1394)

## 0.0.8

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

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

## 0.0.3

## 0.0.2

## 0.0.1
