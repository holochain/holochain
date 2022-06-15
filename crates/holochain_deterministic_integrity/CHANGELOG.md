# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325) 
### Added
- `EntryTypesHelper` helper trait for deserializing to the correct `Entry`.
- `LinkTypesHelper` helper trait for creating `LinkTypeRanges` that fit the current local scope.
### Removed
- `register_entry!` macro as it is no longer needed. Use `hdk_derive::hdk_entry_defs`.
### Changed

## 0.0.8

## 0.0.7

- Fix broken wasm tracing. [PR](https://github.com/holochain/holochain/pull/1389).

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
