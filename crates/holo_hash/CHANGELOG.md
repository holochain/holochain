---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.4.0-dev.2

## 0.4.0-dev.1

## 0.4.0-dev.0

## 0.3.0

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

## 0.3.0-beta-dev.16

## 0.3.0-beta-dev.15

## 0.3.0-beta-dev.14

- **BREAKING CHANGE:** Export error types directly `holo_hash::HoloHashError` instead of path `holo_hash::error::HoloHashError`.

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

## 0.2.0

- Adds more ways to convert between different hash types [\#2283](https://github.com/holochain/holochain/pull/2283)
  - Adds `.into_agent_pub_key() -> Option<AgentPubKey>` for `AnyDhtHash` and `AnyLinkableHash`
  - Adds `TryFrom` impls for all fallible conversions. For instance, if you have a link target (of type AnyLinkableHash), you can now do `let entry_hash: EntryHash = link.target.try_into().unwrap()` if you expect the link target to be an entry hash. (Though we don’t recommend using `.unwrap()` in real code\!)

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

- **BREAKING CHANGE**: `HoloHash::retype()` is removed from the public API, and some `From<AnyDhtHash>` and `From<AnyLinkableHash>` impls were removed. Instances of casting one hash type to another must be done via the remaining From impls, or via `into_primitive()`, `into_entry_hash()`, `into_action_hash()`, etc. for converting from a composite hash to a primitive hash. See [holo\_hash::aliases](https://github.com/holochain/holochain/blob/bf242f00f7ef84cd7f09efc9770dc632f0da4310/crates/holo_hash/src/aliases.rs#L49-L140) for a full listing. [\#2191](https://github.com/holochain/holochain/pull/2191)

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

## 0.0.35

## 0.0.34

## 0.0.33

## 0.0.32

## 0.0.31

- **BREAKING CHANGE** - Refactor: Property `integrity.uid` of DNA Yaml files renamed to `integrity.network_seed`. Functionality has not changed. [\#1493](https://github.com/holochain/holochain/pull/1493)

## 0.0.30

## 0.0.29

## 0.0.28

## 0.0.27

## 0.0.26

## 0.0.25

- Add `Into<AnyLinkableHash>` impl for `EntryHashB64` and `ActionHashB64`
- Add some helpful methods for converting from a “composite” hash type (`AnyDhtHash` or `AnyLinkableHash`) into their respective primitive types:
  - `AnyDhtHash::into_primitive()`, returns an enum
  - `AnyDhtHash::into_entry_hash()`, returns `Option<EntryHash>`
  - `AnyDhtHash::into_action_hash()`, returns `Option<ActionHash>`
  - `AnyLinkableHash::into_primitive()`, returns an enum
  - `AnyLinkableHash::into_entry_hash()`, returns `Option<EntryHash>`
  - `AnyLinkableHash::into_action_hash()`, returns `Option<ActionHash>`
  - `AnyLinkableHash::into_external_hash()`, returns `Option<ExternalHash>`

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

### Fixed

- Crate now builds with `--no-default-features`

## 0.0.5

## 0.0.4

## 0.0.3
