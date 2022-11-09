# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

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

- Adds `basis_hash` index to `DhtOp` table. This makes get queries faster. [\#1143](https://github.com/holochain/holochain/pull/1143)

## 0.0.18

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
