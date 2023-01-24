# Changelog
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).


## [0.0.86] - 2021-09-29

### Added

* `hn-introspect` script to list which holochain packages were pulled in for the nix-shell

#### RSM binaries for Linux
* holochain: 0.0.107
* hc: 0.0.8
* lair-keystore: 0.0.4
* kitsune-p2p-proxy: 0.0.6

Binaries are available for Darwin and Linux on `x86_64-linux`.

#### Configurable holochain/holochain-nixpkgs versions
* Add a section for holochain-nixpkgs to config.nix
* Introduce arguments for choosing the included holochain binaries:

  * holochainVersionId: can be one of "main", "develop", or "custom" as of now.
  * holochainVersion: if `holochainVersionId` is "custom", this specifies a set with holochain source information.
  * include: a set that controls which components to include in the shell
  * rustVersion: allows overriding the nix-shell's rust version. examples:
      * using a specific nightly version: '{ track = "nightly"; version = "2021-07-01"; }'
      * using a specific stable version: '{ track = "stable"; version = "1.53.0"; }'

  Please see the files in _examples/_ for more usage examples.

### Changed
* perf: 4.19 -> 5.4
* perf: 5.4 -> 5.10
* rust: 1.48 -> 1.54
* clippy: 0.0.212 -> 0.1.5*
* Removed the `HC_TARGET_PREFIX` env var in favor of the `NIX_ENV_PREFIX` env var

### Deprecated

### Removed
* n3c
* wasm tools
* rust nightly
* sim2h_server
* newrelic tooling
* saml2aws tool and AWS specific CI jobs
* trycp_server

### Fixed

### Security

## [0.0.85] - 2020-10-02

### Added

### Changed
- Updated to holochain v0.0.52-alpha2

### Deprecated

### Removed

### Fixed

### Security

## [0.0.84] - 2020-09-17

### Added

### Changed
- Updated to holochain v0.0.52-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.83] - 2020-09-10

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.82] - 2020-08-28

### Added

### Changed

- pinned the stable version of rust

### Deprecated

### Removed

### Fixed

### Security

## [0.0.81] - 2020-07-31

### Added

### Changed
- Updated to holochain v0.0.51-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.80] - 2020-07-14

### Added

### Changed
- Updated to holochain v0.0.50-alpha4

### Deprecated

### Removed

### Fixed

### Security

## [0.0.79] - 2020-06-25

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.78] - 2020-06-25

### Added

### Changed

### Deprecated

### Removed

### Fixed

* CI scripts involving Nix installation fixed for Nix installer 2.3.6

### Security

## [0.0.77] - 2020-06-25

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.76] - 2020-06-25

### Added
- hc-happ-scaffold command

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.75] - 2020-05-28

### Added

### Changed
- Updated to holochain v0.0.49-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.74] - 2020-05-14

### Added

- rust: introduce use-stable-rust config variable

### Changed

- Updated to holochain v0.0.48-alpha1
- nix-shell: use bash from nixpkgs explicitly

### Deprecated

### Removed

### Fixed

### Security

## [0.0.73] - 2020-04-09

### Added

### Changed

- Updated to holochain v0.0.47-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.72] - 2020-03-27

### Added

### Changed

- Updated to holochain v0.0.46-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.71] - 2020-03-13

### Added

### Changed

- update to holochain v0.0.45-alpha1 (correctly this time)

### Deprecated

### Removed

### Fixed

### Security

## [0.0.70] - 2020-03-13

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.69] - 2020-03-13

### Added

### Changed

- Updated to holochain v0.0.45-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.68] - 2020-03-11

### Added
- kcov (if linux - technically should work on macOs, but something is broken with the nix package)
- cargo-make (build scripting support - makes it easier to run kcov)
- curl (needed for publishing coverage results to codecov.io)

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.67] - 2020-03-03

### Added

### Changed

- Updated to holochain v0.0.44-alpha3

### Deprecated

### Removed

### Fixed

### Security

## [0.0.66] - 2020-02-11

### Added

### Changed

- Updated to holochain v0.0.43-alpha3

### Deprecated

### Removed

### Fixed

### Security

## [0.0.65] - 2020-01-17

### Added

- added pcre

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.64] - 2020-01-17

### Added

- added hn-release-hook-version-rust-deps

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.63] - 2020-01-17

### Added

- docker builds push to AWS as well as docker hub
- added flamegraph for performance profiling
- added hc-release-hook-publish-crates-io script

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.62] - 2020-01-16

### Added

### Changed

- v0.0.42-alpha5 binaries

### Deprecated

### Removed

### Fixed

### Security

## [0.0.61] - 2020-01-07

### Added

### Changed

### Deprecated

### Removed

### Fixed

- fixed a bug preventing mac to use the nix-shell due to perf

### Security

## [0.0.60] - 2020-01-05

### Added

### Changed

- bump to v0.0.42-alpha3 binaries

### Deprecated

### Removed

### Fixed

### Security

## [0.0.59] - 2020-01-04

### Added

- added perf to nix-shell

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.58] - 2020-01-02

### Added

- added $CARGO_TARGET_DIR and $CARGO_CACHE_RUSTC_INFO to shell from holochain-rust repo

### Changed

- binaries version 0.0.42-alpha2

### Deprecated

### Removed

### Fixed

### Security

## [0.0.57] - 2019-12-31

### Added

- Added dockerfiles for minimal/sim2h_server/trycp_server for binary boxes
- Added utility scripts to make working with docker easier

### Changed

- Changed docker tags to be {box}.{branch}
- Dockers build on love/master/develop (e.g. holochain/holonix:latest.love)
- Using binaries v0.0.42-alpha1

### Deprecated

### Removed

### Fixed

- trycp_server binary is wrapped with holochain to support nix-env installation

### Security

## [0.0.56] - 2019-12-20

### Added

### Changed

- bump to v0.0.41-alpha4 binaries

### Deprecated

### Removed

### Fixed

### Security

## [0.0.55] - 2019-12-19

### Added

### Changed

- update to v0.0.41-alpha3 binaries

### Deprecated

### Removed

### Fixed

### Security

## [0.0.54] - 2019-12-14

### Added

### Changed

- $HC_TARGET_PREFIX is set in nix shell to $NIX_ENV_PREFIX

### Deprecated

### Removed

### Fixed

### Security

## [0.0.53] - 2019-12-12

### Added

### Changed

- The nix shell now respects any existing value for `$NIX_ENV_PREFIX`

### Deprecated

### Removed

### Fixed

### Security

## [0.0.52] - 2019-12-02

### Added

### Changed

### Deprecated

### Removed

### Fixed

- fix for incorrect rust nightly version in v51

### Security

## [0.0.51] - 2019-12-02

### Added

### Changed

- bump rust to nightly-2019-11-25

### Deprecated

### Removed

### Fixed

### Security

## [0.0.50] - 2019-12-01

### Added

### Changed

- bump to binaries v0.0.40-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.49] - 2019-11-30

### Added

### Changed

- binaries v0.0.39-alpha4

### Deprecated

### Removed

### Fixed

### Security

## [0.0.48] - 2019-11-25

### Added

- Automatic rebuilds of nix/ubuntu/debian dockers on love branch pushes

### Changed

- v0.0.39-alpha2 binaries

### Deprecated

### Removed

### Fixed

### Security

## [0.0.47] - 2019-11-13

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.46] - 2019-11-13

### Added

### Changed

- binaries v0.0.38-alpha14

### Deprecated

### Removed

### Fixed

### Security

## [0.0.45] - 2019-11-12

### Added

### Changed

- binaries version v0.0.38-alpha12

### Deprecated

### Removed

### Fixed

### Security

## [0.0.44] - 2019-11-07

### Added

### Changed

- bump to v0.0.44

### Deprecated

### Removed

### Fixed

- fixed url to github nixpkgs resource for 19.09

### Security

## [0.0.43] - 2019-11-06

### Added

### Changed

- rollback to v0.0.35-alpha7

### Deprecated

### Removed

### Fixed

### Security

## [0.0.42] - 2019-11-06

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.41] - 2019-11-06

### Added

### Changed

- using nodejs 12 instead of 11
- using nixos 19.09 channel

### Deprecated

### Removed

### Fixed

### Security

## [0.0.41] - 2019-11-04

### Added

### Changed
* 0.0.36-alpha1 binaries

### Deprecated

### Removed

### Fixed

### Security

## [0.0.40] - 2019-11-04

### Added

### Changed

- 0.0.35-alpha7 binaries

### Deprecated

### Removed

### Fixed

### Security

## [0.0.39] - 2019-10-25

### Added

### Changed

- version 0.0.34-alpha1 of conductors

### Deprecated

### Removed

### Fixed

### Security

## [0.0.38] - 2019-10-23

### Added

- added saml2aws tool

### Changed

- conductor version v0.0.33-alpha5

### Deprecated

### Removed

### Fixed

### Security

## [0.0.37] - 2019-10-14

### Added

- Holochain v0.0.32-alpha2

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.36] - 2019-10-04

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.35] - 2019-10-04

### Added

- added github pages documentation

### Changed

### Deprecated

### Removed

- remove $HC_TARGET_PREFIX (moved to holochain-rust)

### Fixed

### Security

## [0.0.34] - 2019-09-18

### Added

### Changed

- core v0.0.30-alpha6

### Deprecated

### Removed

### Fixed

### Security

## [0.0.33] - 2019-09-17

### Added

### Changed

- bump to core v0.0.30-alpha5

### Deprecated

### Removed

### Fixed

### Security

## [0.0.32] - 2019-09-16

### Added

### Changed

- Moved hc-rust-manifest-* to hn-rust-manifest-*
- holochain-rust version 0.0.30-alpha2

### Deprecated

### Removed

- Removed all hc-* commands

### Fixed

### Security

## [0.0.31] - 2019-09-12

### Added

- Added `watch` cli commands
- $USER is set in Dockerfile
- hc cli tool has dependencies set e.g. for nix-env installations

### Changed

- conductor and cli versions 0.0.29-alpha2

### Deprecated

### Removed

- qt removed

### Fixed

- `hc-rust-manifest-list-unpinned` won't traverse `.cargo/` anymore which resulted in false positives [PR#58](https://github.com/holochain/holonix/pull/58)

### Security

## [0.0.30] - 2019-08-18

### Added
- Bumped Holochain version to 0.0.28-alpha1

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.29] - 2019-08-12

### Added

### Changed


### Deprecated

### Removed

### Fixed
-Fixing release hashes for linux

### Security

## [0.0.29] - 2019-08-12

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.29] - 2019-08-12

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.29] - 2019-08-12

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.29] - 2019-08-12

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.29] - 2019-08-12

### Added

### Changed

- Fixed holonix release

### Deprecated

### Removed

### Fixed

### Security

## [0.0.28] - 2019-08-09

### Added

### Changed

- bump holochain and hc binaries to `0.0.27-alpha1`

### Deprecated

### Removed

### Fixed

### Security

## [0.0.27] - 2019-08-05

### Added

### Changed

- bump holochain and hc binaries to `0.0.26-alpha1`

### Deprecated

### Removed

### Fixed

### Security

## [0.0.26] - 2019-08-05

### Added

### Changed

### Deprecated

### Removed

### Fixed

- Fixed github release process

### Security

## [0.0.25] - 2019-08-05

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.24] - 2019-08-05

### Added

### Changed

### Deprecated

### Removed

### Fixed

- Fixed holonix release process

### Security

## [0.0.23] - 2019-08-05

### Added

- added `RUST_BACKTRACE=1` to nix shell for rust

### Changed

### Deprecated

### Removed

- Removed conductor specific bin scripts

### Fixed

- Fixed the github release hook by removing `-v`

### Security

## [0.0.22] - 2019-07-30

### Added

- Self tests command `hn-test`
- Mac testing on Circle CI
- `bats` for bash testing
- export `$TMP` and `$TMPDIR` in nix shell as `/tmp/tmp.XXXXXXXXXX`

### Changed

- Upgraded github-release for darwin support
- Circle CI runs `hn-test`
- Updated to holochain-rust v0.0.25-alpha1

### Deprecated

### Removed

### Fixed

### Security

## [0.0.21] - 2019-07-16

### Added

### Changed

- bump holochain and hc binaries to `0.0.24-alpha2`

### Deprecated

### Removed

### Fixed

### Security

## [0.0.20] - 2019-07-15

### Added

- add wabt to rust/wasm/default.nix

### Changed

- bumped rust nightly to `2019-07-14`

### Deprecated

### Removed

- many core-only scripts moved to core

### Fixed

### Security

## [0.0.19] - 2019-07-12

### Added

### Changed

- bumped to `0.0.23-alpha1` binaries for conductor and cli

### Deprecated

### Removed

### Fixed

- fixed missing config in default.nix

### Security

## [0.0.18] - 2019-07-09

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.17] - 2019-07-09

### Added

### Changed

- rust version hook no longer requires previous version

### Deprecated

### Removed

### Fixed

### Security

## [0.0.16] - 2019-07-09

### Added

- release hooks for preflight, version, publish

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.15] - 2019-07-09

### Added

- added an extension point for buildInputs in example.config.nix

### Changed

- moved holonix configuration to example.config.nix

### Deprecated

### Removed

### Fixed

- fixed touching missing changelog files early enough in release

### Security

## [0.0.14] - 2019-07-09

### Added

### Changed

### Deprecated

### Removed

### Fixed

- fixed a bug where releases fail if changelog files don't already exist

### Security

## [0.0.13] - 2019-07-09

### Added

### Changed

- example.default.nix points to holonix 0.0.12

### Deprecated

### Removed

### Fixed

### Security

## [0.0.12] - 2019-07-09

### Added

### Changed

### Deprecated

### Removed

### Fixed

- release command no longer references medium
- github template no longer includes legacy placeholder

### Security

## [0.0.11] - 2019-07-09

### Added

### Changed

### Deprecated

### Removed

### Fixed

- github releases are deployed correctly

### Security

## [0.0.10] - 2019-07-09

### Added

### Changed

### Deprecated

### Removed

### Fixed

- fixed release cutting without github syncs

### Security

## [0.0.9] - 2019-07-09

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.8] - 2019-07-09

### Added

- scripts to update github repository for cut releases

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.8] - 2019-07-09

### Added

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.7] - 2019-07-09

### Added

- added hn-release-cut

### Changed

### Deprecated

### Removed

### Fixed

### Security

## [0.0.6] - 2019-07-09

### Added

- added example.default.nix and example.config.nix to dogfood downstream workflows

### Changed

### Deprecated

### Removed

### Fixed

### Security

## 2019-07-09T04:19:14+00:00

### Added

### Changed

- fallback versioning changelog headings use ISO seconds granularity

### Deprecated

### Removed

### Fixed

### Security

## 2019-07-09

### Added

- ability to pass config into root default.nix from consumers
- hn-release-changelog command
