---
default_semver_increment_mode: !pre_minor beta-rc
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.1.0-beta-rc.0

## 0.0.58

## 0.0.57

## 0.0.56

## 0.0.55

**BREAKING CHANGE**: Rename `AuthorizeZomeCallSigningKey` to `GrantZomeCallCapability` & remove parameter `provenance`. [\#1647](https://github.com/holochain/holochain/pull/1647)

## 0.0.54

## 0.0.53

## 0.0.52

## 0.0.51

## 0.0.50

- Revised the changelog for 0.0.48 to note that changes to `ChainQueryFilter` in that version were breaking changes, please read the log for that version for more detail.

## 0.0.49

## 0.0.48

- Add function to set DNA name. [\#1547](https://github.com/holochain/holochain/pull/1547)
- **BREAKING CHANGE** - `ChainQueryFilter` gets a new field, which may cause DNAs built with prior versions to break due to a deserialization error. Rebuild your DNA if so.
- There is now a `ChainQueryFilter::descending()` function which will cause the query results to be returned in descending order. This can be reversed by calling `ChainQueryFilter::ascending()`. The default order is still ascending. [\#1539](https://github.com/holochain/holochain/pull/1539)

## 0.0.47

## 0.0.46

## 0.0.45

## 0.0.44

## 0.0.43

## 0.0.42

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## 0.0.41

## 0.0.40

## 0.0.39

## 0.0.38

## 0.0.37

## 0.0.36

- Bump wasmer to 0.0.80 [\#1386](https://github.com/holochain/holochain/pull/1386)

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `ZomeDef` now holds dependencies for the zome.
- `EntryDefLocation` is either an `EntryDefIndex` or a `CapClaim` or a `CapGrant`.

### Changed

- Zomes are now generic over integrity and coordinator.

- `ZomeDef` is now wrapped in either `IntegrityZomeDef` or `CoordinatorZomeDef`.

- `GetLinksInput` takes a `LinkTypeRanges` for filtering on `LinkType`.

- `CreateInput` takes an `EntryDefLocation` for and an `EntryVisibility` for the entry.

- `UpdateInput` doesn’t take a `CreateInput` anymore.

- `UpdateInput` takes an `Entry` and `ChainTopOrdering`.

- `DnaDef` has split zomes into integrity and coordinator.

- `DnaDef` coordinator zomes do not change the `DnaHash`.

- Docs: Describe init callback and link to WASM examples [\#1418](https://github.com/holochain/holochain/pull/1418)

## 0.0.35

## 0.0.34

## 0.0.33

## 0.0.32

- Docs: Fix intra-doc links in all crates [\#1323](https://github.com/holochain/holochain/pull/1323)

## 0.0.31

## 0.0.30

## 0.0.29

## 0.0.28

## 0.0.27

## 0.0.26

## 0.0.25

- Adds the `Op` type which is used in the validation callback. [\#1212](https://github.com/holochain/holochain/pull/1212)
- Adds the `SignedHashed<T>` type for any data that can be signed and hashed.
- BREAKING CHANGE: Many hashing algorithms can now be specified although only the `Entry` hash type does anything yet [\#1222](https://github.com/holochain/holochain/pull/1222)

## 0.0.24

## 0.0.23

## 0.0.22

## 0.0.21

## 0.0.20

- BREAKING CHANGE: Range filters on chain queries are now INCLUSIVE and support hash bounds [\#1142](https://github.com/holochain/holochain/pull/1142)
- BREAKING CHANGE: Chain queries now support restricting results to a list of entry hashes [\#1142](https://github.com/holochain/holochain/pull/1142)

## 0.0.19

## 0.0.18

## 0.0.17

- BREAKING CHANGE: Add all function names in a wasm to the zome info [\#1081](https://github.com/holochain/holochain/pull/1081)
- BREAKING CHANGE: Added a placeholder for zome properties on zome info [\#1080](https://github.com/holochain/holochain/pull/1080)

## 0.0.16

## 0.0.15

- `HeaderHashes` no longer exists [PR1049](https://github.com/holochain/holochain/pull/1049)
- `HeaderHashedVec` no longer exists [PR1049](https://github.com/holochain/holochain/pull/1049)

## 0.0.14

## 0.0.13

- `CallInfo` now has `as_at` on it [PR 1047](https://github.com/holochain/holochain/pull/1047)
- Removed `Links` in favour of `Vec<Link>` [PR 1012](https://github.com/holochain/holochain/pull/1012)
- Added zome names to `dna_info` [PR 1052](https://github.com/holochain/holochain/pull/1052)

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

### Added

- Added `Schedule` enum to define schedules

## 0.0.8

## 0.0.7

## 0.0.6

### Changed

- `CreateInput`, `DeleteInput`, `DeleteLinkInput` structs invented for zome io
- `EntryDefId` merged into `CreateInput`

### Added

- `ChainTopOrdering` enum added to define chain top ordering behaviour on write

## 0.0.5

### Added

- Countersigning related functions and structs

## 0.0.4

## 0.0.3

### Changed

- `Signature` is a 64 byte ‘secure primitive’

## 0.0.2-alpha.1
