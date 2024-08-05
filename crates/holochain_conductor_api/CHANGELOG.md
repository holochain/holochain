---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.4.0-dev.13

## 0.4.0-dev.12

## 0.4.0-dev.11

## 0.4.0-dev.10

## 0.4.0-dev.9

## 0.4.0-dev.8

## 0.4.0-dev.7

## 0.4.0-dev.6

- *BREAKING* Updates holochain to use the new SBD server architecture for WebRTC signaling. If you were previously running your own tx5-signal-srv server for signaling, you will need to switch to [sbd-server](https://crates.io/crates/sbd-server). If you were using the holo-provided `wss://signal.holo.host`, you must switch to `wss://sbd-0.main.infra.holo.host`. See the module level documentation at [/crates/holochain\_conductor\_api/src/config/conductor.rs](/crates/holochain_conductor_api/src/config/conductor.rs) for a commented example holochain conductor configuration file. [\#3842](https://github.com/holochain/holochain/pull/3842)

## 0.4.0-dev.5

## 0.4.0-dev.4

## 0.4.0-dev.3

## 0.4.0-dev.2

## 0.4.0-dev.1

## 0.4.0-dev.0

## 0.3.0

## 0.3.0-beta-dev.47

## 0.3.0-beta-dev.46

## 0.3.0-beta-dev.45

## 0.3.0-beta-dev.44

## 0.3.0-beta-dev.43

## 0.3.0-beta-dev.42

## 0.3.0-beta-dev.41

## 0.3.0-beta-dev.40

## 0.3.0-beta-dev.39

## 0.3.0-beta-dev.38

## 0.3.0-beta-dev.37

## 0.3.0-beta-dev.36

## 0.3.0-beta-dev.35

## 0.3.0-beta-dev.34

- Added `DumpConductorState` admin method

## 0.3.0-beta-dev.33

## 0.3.0-beta-dev.32

## 0.3.0-beta-dev.31

## 0.3.0-beta-dev.30

## 0.3.0-beta-dev.29

## 0.3.0-beta-dev.28

## 0.3.0-beta-dev.27

## 0.3.0-beta-dev.26

## 0.3.0-beta-dev.25

## 0.3.0-beta-dev.24

- Test: Add tests to App and Admin API to prevent unnoticed changes in serialization from breaking these interfaces.

## 0.3.0-beta-dev.23

## 0.3.0-beta-dev.22

## 0.3.0-beta-dev.21

## 0.3.0-beta-dev.20

## 0.3.0-beta-dev.19

## 0.3.0-beta-dev.18

## 0.3.0-beta-dev.17

Adds `ignore_genesis_failure` field to InstallApp arguments. The default is `false`, and can only use this with the CHC feature. [2612](https://github.com/holochain/holochain/pull/2612)

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

- Add links to concepts documentation to the conductor API module.

## 0.2.0

## 0.2.0-beta-rc.7

## 0.2.0-beta-rc.6

## 0.2.0-beta-rc.5

- `StorageBlob` is an enum that serialized to camel case named variants. Renames all variants to snake case now.

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

- Adds new functionality to the conductor admin API which returns disk storage information. The storage used by apps is broken down into blobs which are being used by one or more app.

## 0.2.0-beta-rc.2

- `AppInfo` now includes a copy of the `AppManifest` which was used to install the app. This can be used to reinstall the same app under a different agent in the same conductor without needing to supply the original DNA files. [\#2157](https://github.com/holochain/holochain/pull/2157)

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

- Reject creation of duplicate clone cells. It was possible to create a clone cell with a DNA hash identical to an already existing DNA. [\#1997](https://github.com/holochain/holochain/pull/1997)
- Adds doc comments for `StemCell`, `ProvisionedCell` and `CloneCell` structs
- Various methods may return a `CellMissing` error if an operation is performed on a disabled cell. Now such calls will return `CellDisabled` to differentiate between a truly missing cell and one that’s just disabled. [\#2092](https://github.com/holochain/holochain/pull/2092)
- Enabling a clone cell that’s already enabled or disabling a clone cell that’s already disabled would previously return a `CloneCellNotFound` error. Now, in those cases, nothing happens and a successful result is returned. [\#2093](https://github.com/holochain/holochain/pull/2093)
- Extend `NetworkInfo` call with several data points related to peer network size and activity. [\#2183](https://github.com/holochain/holochain/pull/2183)

## 0.1.0

## 0.1.0-beta-rc.4

- **BREAKING CHANGE**: `CreateCloneCell` returns `ClonedCell` instead of `InstalledCell`.
- **BREAKING CHANGE**: `EnableCloneCell` returns `ClonedCell` instead of `InstalledCell`.
- **BREAKING CHANGE**: Remove unused call `AdminRequest::StartApp`.
- **BREAKING CHANGE**: `Cell` is split up into `ProvisionedCell` and `ClonedCell`.
- **BREAKING CHANGE**: `CellInfo` variants are renamed to snake case during serde.
- Return additional field `agent_pub_key` in `AppInfo`.

## 0.1.0-beta-rc.3

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

- Fix error while installing app and return app info of newly installed app. [\#1725](https://github.com/holochain/holochain/pull/1725)

## 0.1.0-beta-rc.0

- **BREAKING CHANGE**: Remove deprecated Admin and App API calls.

- **BREAKING CHANGE**: Remove call `InstallApp`.

- **BREAKING CHANGE**: Rename `InstallAppBundle` to `InstallApp`.

- **BREAKING CHANGE**: Rename `ZomeCall` to `CallZome`. [\#1707](https://github.com/holochain/holochain/pull/1707)

- **BREAKING CHANGE**: Rename ArchiveCloneCell to DisableCloneCell.

- **BREAKING CHANGE**: Rename RestoreArchivedCloneCell to EnableCloneCell.

- **BREAKING CHANGE**: Move EnableCloneCell to App API.

- **BREAKING CHANGE**: Refactor DeleteCloneCell to delete a single disabled clone cell. [\#1704](https://github.com/holochain/holochain/pull/1704)

- **BREAKING CHANGE**: Refactor `AppInfo` to return all cells and DNA modifiers.

- **BREAKING CHANGE**: Rename `RequestAgentInfo` to `AgentInfo`. [\#1719](https://github.com/holochain/holochain/pull/1719)

## 0.0.72

## 0.0.71

## 0.0.70

## 0.0.69

## 0.0.68

## 0.0.67

## 0.0.66

## 0.0.65

## 0.0.64

## 0.0.63

## 0.0.62

## 0.0.61

## 0.0.60

## 0.0.59

- Include cloned cells in App API call `AppInfo`. [\#1547](https://github.com/holochain/holochain/pull/1547)
- **BREAKING CHANGE:** The `AddRecords` admin api method has been changed to `GraftRecords`, and the functionality has changed accordingly. See the docs for that method to understand the changes.
  - In short, the `truncate` parameter has been removed. If you desire that functionality, simply pass a fully valid chain in for “grafting”, which will have the effect of removing all existing records. If you just want to append records to the existing chain, just pass in a collection of new records, with the first one pointing to the last existing record.

## 0.0.58

## 0.0.57

## 0.0.56

## 0.0.55

## 0.0.54

## 0.0.53

## 0.0.52

## 0.0.51

## 0.0.50

## 0.0.49

## 0.0.48

## 0.0.47

## 0.0.46

## 0.0.45

## 0.0.44

## 0.0.43

## 0.0.42

## 0.0.41

- Docs: Unify and clean up docs for admin and app interface and conductor config. [\#1391](https://github.com/holochain/holochain/pull/1391)

## 0.0.40

## 0.0.39

## 0.0.38

## 0.0.37

- Docs: Fix intra-doc links in crates `holochain_conductor_api` and `holochain_state` [\#1323](https://github.com/holochain/holochain/pull/1323)

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

- Adds the ability to manually insert elements into a source chain using the `AdminRequest::AddElements` command. Please check the docs and PR for more details / warnings on proper usage. [\#1166](https://github.com/holochain/holochain/pull/1166)

## 0.0.21

## 0.0.20

## 0.0.19

## 0.0.18

## 0.0.17

- **BREAKING CHANGES**: db\_sync\_level changes to db\_sync\_strategy. Options are now `Fast` and `Resilient`. Default is `Fast` and should be the standard choice for most use cases. [\#1130](https://github.com/holochain/holochain/pull/1130)

## 0.0.16

## 0.0.15

## 0.0.14

## 0.0.13

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

## 0.0.8

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

- BREAKING: CONDUCTOR CONFIG CHANGE–related to update to lair 0.0.3
  - `passphrase_service` is now required
    - The only implemented option is `danger_insecure_from_config`

#### Example

``` yaml
passphrase_service:
  type: danger_insecure_from_config
  passphrase: "foobar"
```

## 0.0.2

## 0.0.1
