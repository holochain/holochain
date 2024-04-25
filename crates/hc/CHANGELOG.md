---
default_semver_increment_mode: !pre_patch rc
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.2.8-rc.1

## 0.2.8-rc.0

## 0.2.7

## 0.2.7-rc.1

## 0.2.7-rc.0

## 0.2.6

## 0.2.6-rc.0

## 0.2.5

## 0.2.5-rc.1

## 0.2.5-rc.0

## 0.2.4

## 0.2.4-rc.0

## 0.2.3

## 0.2.3-rc.1

## 0.2.3-rc.0

## 0.2.3-beta-rc.1

## 0.2.3-beta-rc.0

## 0.2.2

## 0.2.2-beta-rc.2

- Adds `chc` feature which is recommended if you want to work with a Holochain instance that is built with its `chc` feature. If you are not using CHC you can safely ignore this feature.

## 0.2.2-beta-rc.1

## 0.2.2-beta-rc.0

## 0.2.1

## 0.2.1-beta-rc.0

- Improved documentation in README, code comments, help text, and error messages.
- Updated from structopt 0.3 to clap 4. [\#2125](https://github.com/holochain/holochain/pull/2125)
- `hc signal-srv` is now `hc run-local-services` and runs both a webrtc signaling server, and the holochain bootstrap server locally. [\#2353](https://github.com/holochain/holochain/pull/2353)

## 0.2.1-beta-dev.0

## 0.2.0

## 0.2.0-beta-rc.7

- Adds a new `hc signal-srv` command to run a local holochain webrtc signal server that can be passed into a command like `hc sandbox generate network webrtc ws://127.0.0.1:xxx`. [\#2265](https://github.com/holochain/holochain/pull/2265)

## 0.2.0-beta-rc.6

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

- Adds new commands to the `hc` CLI which print out JSON schemas for DNA, hApp and web hApps. Use `hc dna schema`, `hc app schema` and `hc web-app schema` to print schemas which can be saved and used as editing aids in your IDE.

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.4

## 0.1.0-beta-rc.3

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

## 0.0.71

- Added handling of `hc` extensions. This allows for existing executables in the system whose names match `hc-<COMMAND>` to be executed with `hc <COMMAND>`.

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

## 0.0.40

## 0.0.39

## 0.0.38

## 0.0.37

## 0.0.36

## 0.0.35

## 0.0.34

## 0.0.33

## 0.0.32

- Fixed broken links in Rust docs [\#1284](https://github.com/holochain/holochain/pull/1284)

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

## 0.0.14

## 0.0.13

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

## 0.0.8

## 0.0.7

- Added the `hc web-app` sub-command for bundling up a UI with a previously created hApp bundle.  It uses the same same behavior as `hc dna` and `hc app` to specify the .yaml manifest file.

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

### Removed

- temporarily removed `install_app` from `hc`: its not clear if we should restore yet as mostly should be using `install_app_bundle` [\#665](https://github.com/holochain/holochain/pull/665)
