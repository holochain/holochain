---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.5.0-dev.1

## 0.5.0-dev.0

## 0.4.0

## 0.4.0-dev.28

## 0.4.0-dev.27

## 0.4.0-dev.26

## 0.4.0-dev.25

## 0.4.0-dev.24

## 0.4.0-dev.23

## 0.4.0-dev.22

## 0.4.0-dev.21

- Add `no-dpki` subcommand flag to the sandbox `generate` and `create` calls.

## 0.4.0-dev.20

## 0.4.0-dev.19

## 0.4.0-dev.18

## 0.4.0-dev.17

## 0.4.0-dev.16

## 0.4.0-dev.15

## 0.4.0-dev.14

## 0.4.0-dev.13

## 0.4.0-dev.12

## 0.4.0-dev.11

## 0.4.0-dev.10

## 0.4.0-dev.9

- Add `dump-network-metrics` and `dump-network-stats` to the sandbox admin calls.

## 0.4.0-dev.8

## 0.4.0-dev.7

## 0.4.0-dev.6

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

- Make `hc-sandbox call` support the `--force_admin_ports`/`-f` flag for specifying which admin ports to connect to. This takes precedence over the `--running`/`-r` flag which exists on the `call` subcommand. So you could still write `hc-sandbox -f 1234 call -r 5678` but the sandbox will connect to the admin port at 1234 instead of 5678.

## 0.3.0-beta-dev.42

## 0.3.0-beta-dev.41

## 0.3.0-beta-dev.40

## 0.3.0-beta-dev.39

## 0.3.0-beta-dev.38

## 0.3.0-beta-dev.37

## 0.3.0-beta-dev.36

## 0.3.0-beta-dev.35

## 0.3.0-beta-dev.34

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

## 0.3.0-beta-dev.23

## 0.3.0-beta-dev.22

## 0.3.0-beta-dev.21

## 0.3.0-beta-dev.20

## 0.3.0-beta-dev.19

## 0.3.0-beta-dev.18

## 0.3.0-beta-dev.17

- `hc sandbox generate` and `hc sandbox run` now exit when the conductor(s) failed to spawn. previously it would wait for the user to cancel manually. [\#2747](https://github.com/holochain/holochain/pull/2747)

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

- updated comment in src/cli.rs to clarify use of –force-admin-ports

- Improved documentation in README, code comments, help text, and error messages.

- Updated from structopt 0.3 to clap 4. [\#2125](https://github.com/holochain/holochain/pull/2125)

- **BREAKING**: In the course of updates, a bug was discovered which necessitated a breaking change; the short arg for `--holochain-path` used in `hc sandbox` subcommand has changed from `-h` to `-H` to resolve a conflict with the short arg for `--help`. [\#2125](https://github.com/holochain/holochain/pull/2125)

## 0.2.0

## 0.2.0-beta-rc.6

## 0.2.0-beta-rc.5

- Add new option `in-process-lair` to `hc sandbox generate` which causes the generated conductor config to specify an in-process lair. This comes with an associated change to make `hc sandbox run` respect the conductor configuration and only launch a lair instance when required.

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

- Fix bug in `hc sandbox generate`, where a comma-separated argument passed to the `--directories` option was treated as a single directory name. [\#2080](https://github.com/holochain/holochain/pull/2080)

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.0

## 0.0.66

## 0.0.65

## 0.0.64

## 0.0.63

## 0.0.62

## 0.0.61

## 0.0.60

## 0.0.59

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

- **BREAKING CHANGE** - `hc sandbox` updated to use new (0.y.z) lair api. Any old sandboxes will no longer function. It is recommended to create new sandboxes, as there is not a straight forward migration path. To migrate: [dump the old keys](https://github.com/holochain/lair/blob/v0.0.11/crates/lair_keystore/src/bin/lair-keystore/main.rs#L38) -\> [write a utility to re-encode them](https://github.com/holochain/lair/tree/hc_seed_bundle-v0.1.2/crates/hc_seed_bundle) -\> [then import them to the new lair](https://github.com/holochain/lair/tree/lair_keystore-v0.2.0/crates/lair_keystore#lair-keystore-import-seed---help) – [\#1515](https://github.com/holochain/holochain/pull/1515)

## 0.0.46

## 0.0.45

- BREAKING CHANGE - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## 0.0.44

## 0.0.43

## 0.0.42

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

- `hc sandbox` command for installing happs was limited to 16mb websocket message limit and would error if given a large happ bundle. now it won’t.  [\#1322](https://github.com/holochain/holochain/pull/1322)
- Fixed broken links in Rust docs [\#1284](https://github.com/holochain/holochain/pull/1284)

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

## 0.0.14

## 0.0.13

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

## 0.0.8

## 0.0.7

## 0.0.6

- Added `UninstallApp` command.

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2
