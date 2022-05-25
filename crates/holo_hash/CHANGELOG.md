# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.0.26

## 0.0.25

- Add `Into<AnyLinkableHash>` impl for `EntryHashB64` and `HeaderHashB64`
- Add some helpful methods for converting from a “composite” hash type (`AnyDhtHash` or `AnyLinkableHash`) into their respective primitive types:
  - `AnyDhtHash::into_primitive()`, returns an enum
  - `AnyDhtHash::into_entry_hash()`, returns `Option<EntryHash>`
  - `AnyDhtHash::into_header_hash()`, returns `Option<HeaderHash>`
  - `AnyLinkableHash::into_primitive()`, returns an enum
  - `AnyLinkableHash::into_entry_hash()`, returns `Option<EntryHash>`
  - `AnyLinkableHash::into_header_hash()`, returns `Option<HeaderHash>`
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
