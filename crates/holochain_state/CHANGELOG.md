---
default_semver_increment_mode: !pre_minor beta-dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

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

- Fix: Maximum one unrestricted cap grant was looked up from the source chain to authorize a remote call. Now all unrestricted cap grants are checked for validity.

## 0.3.0-beta-dev.25

## 0.3.0-beta-dev.24

- Change the license from CAL-1.0 to Apache-2.0.

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

## 0.3.0-beta-dev.13

## 0.3.0-beta-dev.12

## 0.3.0-beta-dev.11

## 0.3.0-beta-dev.10

- fix: in a scenario where two agents create a cell from the same DNA in the same conductor, cap grant lookup for zome calls succeeded erroneously for any calling agent. The cap grant author was not taken into consideration for the lookup, only the cap secret or the unrestricted cap entry. Fixed by filtering the lookup by cap grant author.

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

## 0.2.0-beta-rc.7

## 0.2.0-beta-rc.6

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

- Optimize capability grant verification during zome calls. This speeds up all remote calls, under which fall calls with a cap secret from clients other than the Launcher. Previously hundreds of calls would slow down response time noticeably because of grant verification. Now thousands of calls (rather thousands of records) won’t affect grant verification by more than a millisecond. [\#2097](https://github.com/holochain/holochain/pull/2097)

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.3

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

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

## 0.0.21

## 0.0.20

## 0.0.19

## 0.0.18

## 0.0.17

- Some databases can handle corruption by wiping the db file and starting again. [\#1039](https://github.com/holochain/holochain/pull/1039).

## 0.0.16

## 0.0.15

## 0.0.14

- BREAKING CHANGE. Source chain `query` will now return results in action sequence order ascending.

## 0.0.13

## 0.0.12

## 0.0.11

## 0.0.10

## 0.0.9

- Fixed a bug when creating an entry with `ChainTopOrdering::Relaxed`, in which the action was created and stored in the Source Chain, but the actual entry was not.
- Geneis ops will no longer run validation for the authored node and only genesis self check will run. [\#995](https://github.com/holochain/holochain/pull/995)

## 0.0.8

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
