---
default_semver_increment_mode: !pre_minor beta-dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.3.0-beta-dev.27

## 0.3.0-beta-dev.26

## 0.3.0-beta-dev.25

## 0.3.0-beta-dev.24

## 0.3.0-beta-dev.23

## 0.3.0-beta-dev.22

- Enhance source backoff logic. The fetch pool used to give a source a 5 minute pause if it failed to serve an op before using the source again. Now the failures to serve by sources is tracked across the pool. Sources that fail too often will be put on a backoff to give them a chance to deal with their current workload before we use them again. For hosts that continue to not respond they will be dropped as sources for ops. Ops that end up with no sources will be dropped from the fetch pool. This means that we can stop using resources on ops we will never be able to fetch. If a source appears who is capable of serving the missing ops then they should be re-added to the fetch pool.

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

## 0.3.0-beta-dev.9

## 0.3.0-beta-dev.8

## 0.3.0-beta-dev.7

- Fix an issue with merging fetch contexts where merging an item with a context with an item that did not could result in the removal of the context.
- Fix an issue where duplicate fetch sources would be permitted for a single item.

## 0.3.0-beta-dev.6

## 0.3.0-beta-dev.5

## 0.3.0-beta-dev.4

## 0.3.0-beta-dev.3

## 0.3.0-beta-dev.2

## 0.3.0-beta-dev.1

## 0.3.0-beta-dev.0

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
