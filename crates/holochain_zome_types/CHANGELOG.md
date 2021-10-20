# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/holochain/holochain/holochain_zome_types-v0.0.2-alpha.1...HEAD)

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
