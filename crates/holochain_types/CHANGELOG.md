# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.0.50

## 0.0.49

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## 0.0.48

## 0.0.47

## 0.0.46

## 0.0.45

## 0.0.44

## 0.0.43

## 0.0.42

### Integrity / Coordinator Changes [\#1325](https://github.com/holochain/holochain/pull/1325)

### Added

- `GlobalZomeTypes` type that holds all a dna’s zome types.
- `ToSqlStatement` trait for converting a type to a SQL statement.
- `InlineZomeSet` for creating a set of integrity and coordinator inline zomes.
- `DnaManifest` takes dependencies for coordinator zomes. These are the names of integrity zomes and must be within the same manifest.
- `DnaManifest` verifies that all zome names are unique.
- `DnaManifest` verifies that dependency names exists and are integrity zomes.
- `DnaFile` can hot swap coordinator zomes. Existing zomes are replaced and new zome names are appended.

### Changed

- `RibosomeStore` is now a `RibosomeStore`.
- `DnaManifest` now has an integrity key for all values that will change the dna hash.
- `DnaManifest` now has an optional coordinator key for adding coordinators zomes on install.

## 0.0.41

## 0.0.40

## 0.0.39

## 0.0.38

## 0.0.37

## 0.0.36

## 0.0.35

## 0.0.34

## 0.0.33

## 0.0.32

## 0.0.31

## 0.0.30

## 0.0.29

## 0.0.28

## 0.0.27

## 0.0.26

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

## 0.0.15

- FIX: [Bug](https://github.com/holochain/holochain/issues/1101) that was allowing `HeaderWithoutEntry` to shutdown apps. [\#1105](https://github.com/holochain/holochain/pull/1105)

## 0.0.14

## 0.0.13

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

## 0.0.8

## 0.0.7

- Added helper functions to `WebAppBundle` and `AppManifest` to be able to handle these types better in consuming applications.

## 0.0.6

- Added `WebAppManifest` to support `.webhapp` bundles. This is necessary to package hApps together with web UIs, to export to the Launcher and Holo.

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1

### Changed

- BREAKING: All references to `"uuid"` in the context of DNA has been renamed to `"uid"` to reflect that these IDs are not universally unique, but merely unique with regards to the zome code (the genotype) [\#727](https://github.com/holochain/holochain/pull/727)
