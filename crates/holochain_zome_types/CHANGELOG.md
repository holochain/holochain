# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/holochain/holochain/holochain_zome_types-v0.0.2-alpha.1...HEAD)
- Docs: Fix intra-doc links in all crates [#1323](https://github.com/holochain/holochain/pull/1323)

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
