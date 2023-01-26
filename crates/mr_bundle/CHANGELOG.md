# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

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
