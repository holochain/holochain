---
default_semver_increment_mode: !pre_minor beta-dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

- Added `AllowedOrigins` which is intended to be used with `holochain_websocket` for controlling access. It is placed here
  for crates need to know about origins but don't depend on `holochain_websocket`. [#3460](https://github.com/holochain/holochain/pull/3460)

## 0.3.0-beta-dev.37

## 0.3.0-beta-dev.36

## 0.3.0-beta-dev.35

## 0.3.0-beta-dev.34

## 0.3.0-beta-dev.33

## 0.3.0-beta-dev.32

## 0.3.0-beta-dev.31

- Refactor: All logic related to modules and wasmer caching has been moved to `holochain-wasmer`. Consequently functions for wasmer development under iOS need to be imported from there.

## 0.3.0-beta-dev.30

## 0.3.0-beta-dev.29

## 0.3.0-beta-dev.28

## 0.3.0-beta-dev.27

## 0.3.0-beta-dev.26

## 0.3.0-beta-dev.25

## 0.3.0-beta-dev.24

- **BREAKING CHANGE**: A `DnaManifest` and all its sub-fields will now reject unknown fields when deserialized. This will make it harder to provide an invalid DNA manifest to Holochain without realising. For example, coordinator zomes not appearing in your installed hApp because their field was indented to the wrong place. This is not a breaking change for valid manifests but Holochain will now reject more invalid manifests.

## 0.3.0-beta-dev.23

## 0.3.0-beta-dev.22

- Change the license from CAL-1.0 to Apache-2.0.

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

- In the CloneOnly provisioning strategy, `installed_hash` is no longer required (it’s now optional). [\#2600](https://github.com/holochain/holochain/pull/2600)

## 0.3.0-beta-dev.9

## 0.3.0-beta-dev.8

## 0.3.0-beta-dev.7

## 0.3.0-beta-dev.6

## 0.3.0-beta-dev.5

## 0.3.0-beta-dev.4

- **BREAKING CHANGE**: `DhtOp` now uses a `RecordEntry` instead of `Option<Box<Entry>>` to denote the reason why an Entry was not included, if there is no entry included. You can get an `Option<Entry>` via `RecordEntry::as_option()` or `RecordEntry::into_option()`. `Op` and `FlatOp` (used for validation) are unchanged.

## 0.3.0-beta-dev.3

## 0.3.0-beta-dev.2

## 0.3.0-beta-dev.1

## 0.3.0-beta-dev.0

## 0.2.0

## 0.2.0-beta-rc.7

- `ZomeManifest` now takes a `dylib` argument, with the type `Option<PathBuf>`. It can be safely ignored in cases other than trying to execute on native iOS. It is used with artifacts produced by `hc dna pack` when it has been called with the `--dylib-ios` option. [\#2218](https://github.com/holochain/holochain/pull/2218)

## 0.2.0-beta-rc.6

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

- BREAKING: AppManifest’s `version` now only accepts a single optional DNA hash value, whereas previously it could accept a list of values.
- AppManifest’s `version` is renamed to `installed_hash` (there is still an alias to `version` for limited backward compatibility, so the old field name will still work).

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.3

## 0.1.0-beta-rc.2

- BREAKING CHANGE - Added zome name to the signal emitted when using `emit_signal`.

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

## 0.0.69

## 0.0.68

## 0.0.67

## 0.0.66

## 0.0.65

- Fixed a bug where DNA modifiers specified in a hApp manifest would not be respected when specifying a `network_seed` in a `InstallAppBundlePayload`. [\#1642](https://github.com/holochain/holochain/pull/1642)

## 0.0.64

## 0.0.63

## 0.0.62

## 0.0.61

- Added `WebAppManifestCurrentBuilder` and exposed it.

## 0.0.60

## 0.0.59

## 0.0.58

- **BREAKING CHANGE**: `network_seed`, `origin_time` and `properties` are combined in a new struct `DnaModifiers`. API calls `RegisterDna`, `InstallAppBundle` and `CreateCloneCell` require this new struct as a substruct under the field `modifiers` now. [\#1578](https://github.com/holochain/holochain/pull/1578)
  - This means that all DNAs which set these fields will have to be rebuilt, and any code using the API will have to be updated (the @holochain/client Javascript client will be updated accordingly).
- **BREAKING CHANGE**: `origin_time` is a required field now in the `integrity` section of a DNA manifest.

## 0.0.57

- Renamed `SweetEasyInline` to `SweetInlineZomes`
- Renamed `InlineZome::callback` to `InlineZome::function`

## 0.0.56

- Add function to add a clone cell to an app. [\#1547](https://github.com/holochain/holochain/pull/1547)

## 0.0.55

## 0.0.54

## 0.0.53

## 0.0.52

## 0.0.51

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
