# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
