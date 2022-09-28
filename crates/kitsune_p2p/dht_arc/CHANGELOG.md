# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.0.16

## 0.0.15

## 0.0.14

## 0.0.13

## 0.0.12

## 0.0.11

- **BREAKING** Arcs are now “unidirectional”, meaning rather than the agent location defining the centerpoint of the storage arc, the agent location defines the left edge of the arc.

This is a huge change, particularly to gossip behavior. With bidirectional arcs, when peers have roughly equivalently sized arcs, half of the peers who have overlapping arcs will not see each other or gossip with each other because their centerpoints are not contained within each others’ arcs. With unidirectional arcs, this problem is removed at the expense of making peer discovery asymmmetrical, which we have found to have no adverse effects.

## 0.0.10

## 0.0.9

## 0.0.8

- New arc resizing algorithm based on `PeerViewBeta`
- In both arc resizing algorithms, instead of aiming for the ideal target arc size, aim for an ideal range. This slack in the system allows all agents to converge on their target more stably, with less oscillation.

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
