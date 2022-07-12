# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
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
