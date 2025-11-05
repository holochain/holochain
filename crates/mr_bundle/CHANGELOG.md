---
default_semver_increment_mode: !pre_minor rc
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.6.0-dev.8

## 0.6.0-dev.7

## 0.6.0-dev.6

## 0.6.0-dev.5

## 0.6.0-dev.4

## 0.6.0-dev.3

## 0.6.0-dev.2

## 0.6.0-dev.1

- Major rework of Mr. Bundle:
  - Collapsed multiple error types into a single `MrBundleError` type.
  - Added context for I/O errors because they’re unusable when you don’t know which file operation might have caused them.
  - Removed `RawBundle`, the purpose it used to fill for working with bundles with an unknown manifest can now be done without a specialized type.
  - Renamed manifest `path` method to `file_name` and changed the return type from `PathBuf` to `&'static str` because that’s how it’s always used.
  - Added a new `generate_resource_ids` method to the `Manifest` type. See documentation for usage.
  - Removed all file system operations from the `Manifest` and `Bundle` types. The same functionality with a simpler interface is now provided by a new `FileSystemBundler` type.
  - The `Bundle` no longer requires complex logic to work with relative paths, so the `new_unchecked` method which bypassed these checks has been removed.
  - Methods that need to accept data as input now prefer `impl std::io::Read` instead of a mixture of `Vec<u8>` and `bytes::Bytes`. `Bytes` can still be passed using its `.reader()` method provided by `bytes::Buf`.
  - The `encode` and `decode` methods of `Bundle` have been renamed to `pack` and `unpack` to better reflect their purpose.
  - The `Location` type has been removed. Although a few Holochain test used the `Path` type, it is not relevant when sharing bundles. The URL type was unimplemented. All resources are now expected to be bundled.

## 0.6.0-dev.0

- **BREAKING CHANGE** `Bundle::encode` now takes `bytes::Bytes` as input, and `Bundle::decode` now returns `bytes::Bytes`.
- **BREAKING CHANGE** `ResourceBytes(Vec<u8>)` is now `ResourceBytes(bytes::Bytes)`.

## 0.5.0

## 0.5.0-rc.0

## 0.5.0-dev.6

## 0.5.0-dev.5

## 0.5.0-dev.4

- Use `rustls-tls` instead of `native-tls-vendored` in reqwest due to compatibility issue with Android platform

## 0.5.0-dev.3

## 0.5.0-dev.2

## 0.5.0-dev.1

## 0.5.0-dev.0

## 0.4.0

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

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.2

- **BREAKING CHANGE:** The `resources` field of bundles was not properly set up for efficient serialization. Bundles built before this change must now be rebuilt. [\#1723](https://github.com/holochain/holochain/pull/1723)
  - Where the actual bytes of the resource were previously specified by a simple sequence of numbers, now a byte array is expected. For instance, in JavaScript, this is the difference between an Array and a Buffer.

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

## 0.0.20

## 0.0.19

## 0.0.18

## 0.0.17

## 0.0.16

## 0.0.15

## 0.0.14

- Fix inconsistent bundle writting due to unordered map of bundle resources

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

## 0.0.2

## 0.0.1
