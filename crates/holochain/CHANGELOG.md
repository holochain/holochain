# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

*Note: Versions 0.0.52-alpha2 and older are part belong to previous iterations of the Holochain architecture and are not tracked here.*

## Unreleased

### Changed

- Apps now have a more complex status. Apps now can be either enabled/disabled as well as running/stopped, the combination of which is captured by three distinctly named states:
  - "Running" (enabled + running) -> The app is running normally
  - "Paused" (enabled + stopped) -> The app is currently stopped due to some minor problem such as failed network access, but will start running again as soon as it's able
  - "Disabled" (disabled + stopped) -> The app is stopped and will remain so until explicitly enabled via `EnableApp` admin method. Apps can be disabled manually via `DisableApp`, or automatically due to an unrecoverable error in a Cell.
- Some admin methods are deprecated due to the app status changes:
  - `ActivateApp` is deprecated in favor of `EnableApp`
  - `DeactivateApp` is deprecated in favor of `DisableApp`

### Added

- `InstallAppBundle` command added to admin conductor API. [#665](https://github.com/holochain/holochain/pull/665)
- `DnaSource` in conductor_api `RegisterDna` call now can take a `DnaBundle` [#665](https://github.com/holochain/holochain/pull/665)
- New admin interface methods:
  - `EnableApp` (replaces `ActivateApp`)
  - `DisableApp` (replaces `DeactivateApp`)
  - `StartApp` (used to attempt to manually restart a Paused app)

### Removed

- BREAKING:  `InstallAppDnaPayload` in admin conductor API `InstallApp` command now only accepts a hash.  Both properties and path have been removed as per deprecation warning.  Use either `RegisterDna` or `InstallAppBundle` instead. [#665](https://github.com/holochain/holochain/pull/665)
- BREAKING: `DnaSource(Path)` in conductor_api `RegisterDna` call now must point to `DnaBundle` as created by `hc dna pack` not a `DnaFile` created by `dna_util` [#665](https://github.com/holochain/holochain/pull/665)

## 0.0.100

This is the first version number for the version of Holochain with a refactored state model (you may see references to it as Holochain RSM).
