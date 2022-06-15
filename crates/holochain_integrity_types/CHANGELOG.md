# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

- Adds ZomeTypeCheck and ParseEntry.
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
- `EntryDefs::entry_def_index_from_id` is removed because it's no longer possible to go from an `EntryDefId` to a `GlobalZomeTypeId` as `EntryDefId` is not globally unique.
- `ZomeInfo::matches_entry_def_id` for the same reason as `EntryDefs::entry_def_index_from_id` 
- `require_validation_type` is removed because it is no longer used.
- `ZomeId` from `CreateLink` as it's no longer needed because `LinkType` is a `GlobalZomeTypeId`.
- `ZomeId` from `AppEntryType` as it's no longer needed because `EntryDefIndex` is a `GlobalZomeTypeId`  
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
