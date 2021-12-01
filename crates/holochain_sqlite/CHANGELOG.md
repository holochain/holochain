# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.0.17

- **BREAKING CHANGES**: All DHT data for the same DNA space is now shared in the same database. All authored data for the same DNA space is also now shared in another database. This requires no changes however data must be manually migrated from the old databases to the new databases. [\#1130](https://github.com/holochain/holochain/pull/1130)

## 0.0.16

## 0.0.15

- Fixes: Bug where database connections would timeout and return `DatabaseError(DbConnectionPoolError(Error(None)))`. [\#1097](https://github.com/holochain/holochain/pull/1097).

## 0.0.14

## 0.0.13

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

- Update to rusqlite 0.26.0 [\#1023](https://github.com/holochain/holochain/pull/1023)
  - provides `bundled-sqlcipher-vendored-openssl` to ease build process on non-windows systems (windows is still using `bundled` which doesn’t provide at-rest encryption).

## 0.0.8

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
