---
semver_increment_mode: patch
default_semver_increment_mode: !pre_patch rc
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.2.7

## 0.2.6

## 0.2.6-rc.0

- Enhance source backoff logic. The fetch pool used to give a source a 5 minute pause if it failed to serve an op before using the source again. Now the failures to serve by sources is tracked across the pool. Sources that fail too often will be put on a backoff to give them a chance to deal with their current workload before we use them again. For hosts that continue to not respond they will be dropped as sources for ops. Ops that end up with no sources will be dropped from the fetch pool. This means that we can stop using resources on ops we will never be able to fetch. If a source appears who is capable of serving the missing ops then they should be re-added to the fetch pool.

## 0.2.5

## 0.2.5-rc.1

## 0.2.5-rc.0

## 0.2.4

## 0.2.4-rc.0

## 0.2.3

## 0.2.3-rc.1

## 0.2.3-rc.0

## 0.2.3-beta-rc.0

## 0.2.2

## 0.2.2-beta-rc.1

## 0.2.2-beta-rc.0

- Fix an issue with merging fetch contexts where merging an item with a context with an item that did not could result in the removal of the context.
- Fix an issue where duplicate fetch sources would be permitted for a single item.

## 0.2.1

## 0.2.1-beta-rc.0

## 0.2.1-beta-dev.0

## 0.2.0

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

## 0.1.0

## 0.1.0-beta-rc.1

## 0.1.0-beta-rc.0

## 0.0.1
