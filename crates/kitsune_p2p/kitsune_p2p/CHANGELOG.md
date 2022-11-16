# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

## 0.0.51

- `rpc_multi` now only actually makes a single request. This greatly simplifies the code path and eliminates a source of network bandwidth congestion, but removes the redundancy of aggregating the results of multiple peers. [\#1651](https://github.com/holochain/holochain/pull/1651)

## 0.0.50

## 0.0.49

## 0.0.48

## 0.0.47

## 0.0.46

## 0.0.45

## 0.0.44

- Fixes a regression where a node can prematurely end a gossip round if their partner signals that they are done sending data, even if the node itself still has more data to send, which can lead to persistent timeouts between the two nodes. [\#1553](https://github.com/holochain/holochain/pull/1553)

## 0.0.43

- Increases all gossip bandwidth rate limits to 10mbps, up from 0.1mbps, allowing for gossip of larger entries
- Adds `gossip_burst_ratio` to `KitsuneTuningParams`, allowing tuning of bandwidth bursts
- Fixes a bug where a too-large gossip payload could put the rate limiter into an infinite loop

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

- Allow TLS session keylogging via tuning param `danger_tls_keylog` = `env_keylog`, and environment variable `SSLKEYLOGFILE` (See kitsune\_p2p crate api documentation). [\#1261](https://github.com/holochain/holochain/pull/1261)

## 0.0.25

- BREAKING: Gossip messages no longer contain the hash of the ops being gossiped. This is a breaking protocol change.
- Removed the unmaintained “simple-bloom” gossip module in favor of “sharded-gossip”

## 0.0.24

## 0.0.23

- Fixes D-01415 holochain panic on startup [\#1206](https://github.com/holochain/holochain/pull/1206)

## 0.0.22

## 0.0.21

## 0.0.20

## 0.0.19

## 0.0.18

## 0.0.17

- Agent info is now published as well as gossiped. [\#1115](https://github.com/holochain/holochain/pull/1115)
- BREAKING: Network wire message has changed format so will not be compatible with older versions. [1143](https://github.com/holochain/holochain/pull/1143).
- Fixes to gossip that allows batching of large amounts of data. [1143](https://github.com/holochain/holochain/pull/1143).

## 0.0.16

## 0.0.15

- BREAKING: Wire message `Call` no longer takes `from_agent`. [\#1091](https://github.com/holochain/holochain/pull/1091)

## 0.0.14

## 0.0.13

## 0.0.12

- BREAKING: Return `ShardedGossipWire::Busy` if we are overloaded with incoming gossip. [\#1076](https://github.com/holochain/holochain/pull/1076)
  - This breaks the current network protocol and will not be compatible with other older versions of holochain (no manual action required).

## 0.0.11

## 0.0.10

- Check local agents for basis when doing a RPCMulti call. [\#1009](https://github.com/holochain/holochain/pull/1009).

## 0.0.9

- Fix rpc\_multi bug that caused all request to wait 3 seconds. [\#1009](https://github.com/holochain/holochain/pull/1009/)
- Fix to gossip’s round initiate. We were not timing out a round if there was no response to an initiate message. [\#1014](https://github.com/holochain/holochain/pull/1014).
- Make gossip only initiate with agents that have info that is not expired. [\#1014](https://github.com/holochain/holochain/pull/1014).

## 0.0.8

### Changed

- `query_gossip_agents`, `query_agent_info_signed`, and `query_agent_info_signed_near_basis` are now unified into a single `query_agents` call in `KitsuneP2pEvent`

## 0.0.7

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
