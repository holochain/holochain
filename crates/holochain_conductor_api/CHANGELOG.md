# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

- **BREAKING CHANGE**: Rename ArchiveCloneCell to DisableCloneCell.
- **BREAKING CHANGE**: Rename RestoreArchivedCloneCell to EnableCloneCell.
- **BREAKING CHANGE**: Move EnableCloneCell to App API.
- **BREAKING CHANGE**: Refactor DeleteCloneCell to delete a single disabled clone cell. [\#1704](https://github.com/holochain/holochain/pull/1704)

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
