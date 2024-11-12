---
default_semver_increment_mode: !pre_minor dev
---
# Changelog

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/). This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## \[Unreleased\]

# Changed
- chore: Kitsune P2P is now its own workspace. All individual changelogs have been merged into this one.
- chore(bin_data): The dependency on `holochain_util` has been removed and replaced with `hex`. Functionality hasn't changed 
  but the dependency on Holochain crates wasn't wanted.
- chore(dht): The dependency on `holochain_trace` has been removed in favour of just using `tracing-subscriber` directly. This was for tests only.
- chore(dht_arc): The dependency on `holochain_trace` has been removed in favour of just using `tracing-subscriber` directly. This was for tests only.
- chore(fetch): The dependency on `holochain_trace` has been removed in favour of just using `tracing-subscriber` directly. This was for tests only.
- chore(kitsune_p2p): The dependency on `holochain_trace` has been removed in favour of just using `tracing-subscriber` directly. This was for tests only.
- chore(kitsune_p2p): Add `metrics_helper` module from the `holochain_trace` crate.

## 0.5.0-dev.4

- kitsune_p2p: Removed `network_type` from `KitsuneP2pConfig`

## 0.5.0-dev.3

## 0.5.0-dev.2

## 0.5.0-dev.1

- mdns: Bump libmdns to 0.9.1 to avoid constantly logged warnings of “Address already in use”

## 0.5.0-dev.0

## 0.4.0

## 0.4.0-dev.23

## 0.4.0-dev.22

## 0.4.0-dev.21

## 0.4.0-dev.20

## 0.4.0-dev.19

## 0.4.0-dev.18

## 0.4.0-dev.17

- kitsune_p2p: minor sbd rate limiting fix - tx5 webrtc buffer size fix - tx5 webrtc connection close fix - better tx5 connection tracing [\#4184](https://github.com/holochain/holochain/pull/4184)

## 0.4.0-dev.16

## 0.4.0-dev.15

## 0.4.0-dev.14

## 0.4.0-dev.13

## 0.4.0-dev.12

## 0.4.0-dev.11

## 0.4.0-dev.10

## 0.4.0-dev.9

- kitsune_p2p: Change the default `DbSyncStrategy` from `Fast` to `Resilient` which should reduce database corruptions. You can override this setting in your conductor config if you wish to go faster and are willing to rebuild your database if it gets corrupted. \#4010

## 0.4.0-dev.8

- kitsune_p2p: **BREAKING** Bumped KITSUNE\_PROTOCOL\_VERSION. This *should* have been bumped with https://github.com/holochain/holochain/pull/3842, but better late than never. [\#3984](https://github.com/holochain/holochain/pull/3984)

## 0.4.0-dev.7

## 0.4.0-dev.6

## 0.4.0-dev.5

## 0.4.0-dev.4

- kitsune_p2p: Fix an issue with delegated publish where delegates were publishing to nodes near the target basis, rather than nodes covering the basis.

## 0.4.0-dev.3

## 0.4.0-dev.2

## 0.4.0-dev.1

## 0.4.0-dev.0

## 0.3.0

## 0.3.0-beta-dev.40

- kitsune_p2p: **BREAKING** - AgentInfo uses a quantized arc instead of an arc half-length to represent DHT coverage. This is a breaking protocol change, nodes on different versions will not be able to gossip with each other.

## 0.3.0-beta-dev.39

## 0.3.0-beta-dev.38

- kitsune_p2p: Kitsune will now close connections in two cases. Firstly, when receiving an updated agent info that indicates that the peer URL for an agent has changed. If there is an open connection to the old peer URL then it will be closed. Secondly, when if a message from a peer fails to decode. This is likely sign that communication between two conductors is not going to work so the connection is closed.

## 0.3.0-beta-dev.37

## 0.3.0-beta-dev.36

## 0.3.0-beta-dev.35

## 0.3.0-beta-dev.34

## 0.3.0-beta-dev.33

- kitsune_p2p: *BREAKING* Adds a preflight check in tx5 which requires the equality of two things: a `KITSUNE_PROTOCOL_VERSION`, which is incremented every time there is a breaking protocol change in Kitsune, and an opaque `user_data` passed in by the host, which allows the host to specify its own compatibility requirements. This allows protocol incompatibilities to be explicitly handled and logged, rather than letting things silently and unpredictably fail in case of a mismatch in protocol datatypes.

## 0.3.0-beta-dev.32

## 0.3.0-beta-dev.31

- kitsune_p2p: *BREAKING* Updates tx5 to a version using new endpoint state logic and a new incompatible protocol. [\#3287](https://github.com/holochain/holochain/pull/3287)

## 0.3.0-beta-dev.30

- kitsune_p2p: Performance improvement by reducing the number of `query_agents` calls used by Kitsune. The host (Holochain conductor) responds to these queries using an in-memory store which is fast but all the queries go through the `ghost_actor` so making an excessive number of calls for the same information reduces the availability of the host for other calls. For a test which sets up 10 spaces (equivalent to a happ running on the host) this change takes the number of host queries for agent info from ~13k to ~1.4k. The removed calls were largely redundant since Kitsune refreshes agent info every 1s anyway so it shouldn’t need to make many further calls between refreshes.

- kitsune_p2p: Minor optimisation when delegate broadcasting ops, the delegated broadcasts will now avoid connecting back to the source. There is currently no way to prevent other agents that were delegated to from connecting to each other but this change takes care of one case.

## 0.3.0-beta-dev.29

## 0.3.0-beta-dev.28

## 0.3.0-beta-dev.27

## 0.3.0-beta-dev.26

## 0.3.0-beta-dev.25

## 0.3.0-beta-dev.24

## 0.3.0-beta-dev.23

- kitsune_p2p: Gossip send failures and target expired events are now logged as warnings rather than errors, and have additional text for clarity. [\#2974](https://github.com/holochain/holochain/pull/2974)

## 0.3.0-beta-dev.22

- kitsune_p2p: Update to a tx5 version that includes go code that is statically linked for all platform that support it. Windows and Android will remain dynamically linked. [\#2967](https://github.com/holochain/holochain/pull/2967)
- kitsune_p2p: Change the license from Apache-2.0 to CAL-1.0.
- kitsune_p2p: Fixed spammy “Recorded initiate|accept with current round already set” warning. [\#3060](https://github.com/holochain/holochain/pull/3060)
- fetch: Enhance source backoff logic. The fetch pool used to give a source a 5 minute pause if it failed to serve an op before using the source again. Now the failures to serve by sources is tracked across the pool. Sources that fail too often will be put on a backoff to give them a chance to deal with their current workload before we use them again. For hosts that continue to not respond they will be dropped as sources for ops. Ops that end up with no sources will be dropped from the fetch pool. This means that we can stop using resources on ops we will never be able to fetch. If a source appears who is capable of serving the missing ops then they should be re-added to the fetch pool.

## 0.3.0-beta-dev.21

- kitsune_p2p: There were some places where parsing an invalid URL would crash kitsune. This is now fixed. [\#2689](https://github.com/holochain/holochain/pull/2689)

## 0.3.0-beta-dev.20

- kitsune_p2p: Augment network stats with holochain agent info correlation [\#2953](https://github.com/holochain/holochain/pull/2953)
- kitsune_p2p: Adjust bootstrap max\_delay from 60 minutes -\> 5 minutes [\#2948](https://github.com/holochain/holochain/pull/2948)

## 0.3.0-beta-dev.19

## 0.3.0-beta-dev.18

## 0.3.0-beta-dev.17

- kitsune_p2p: Add additional configuration options to network\_tuning for setting the allowed ephemeral port range for tx5 connections: tx5\_min\_ephemeral\_udp\_port and tx5\_max\_ephemeral\_udp\_port
- bootstrap_client: Extracted bootstrap client crate from `kitsune_p2p` to allow re-use.

## 0.3.0-beta-dev.16

## 0.3.0-beta-dev.15

## 0.3.0-beta-dev.14

- kitsune_p2p: Resolves several cases where the meta net task would not stop on fatal errors and would not correctly handle other errors [\#2762](https://github.com/holochain/holochain/pull/2762)
- kitsune_p2p: Resolves an issue where a `FetchOp` could skip processing op hashes if getting a topology for the space from the host failed [\#2737](https://github.com/holochain/holochain/pull/2737)
- kitsune_p2p: Adds a warning log if incoming op data pushes are dropped due to a hashing failure on the host [\#2737](https://github.com/holochain/holochain/pull/2737)
- kitsune_p2p: Fixes an issue where sending an unexpected request payload would cause the process to crash [\#2737](https://github.com/holochain/holochain/pull/2737)

## 0.3.0-beta-dev.13

## 0.3.0-beta-dev.12

## 0.3.0-beta-dev.11

## 0.3.0-beta-dev.10

## 0.3.0-beta-dev.9

## 0.3.0-beta-dev.8

## 0.3.0-beta-dev.7

- fetch: Fix an issue with merging fetch contexts where merging an item with a context with an item that did not could result in the removal of the context.
- fetch: Fix an issue where duplicate fetch sources would be permitted for a single item.

## 0.3.0-beta-dev.6

## 0.3.0-beta-dev.5

## 0.3.0-beta-dev.4

## 0.3.0-beta-dev.3

## 0.3.0-beta-dev.2

## 0.3.0-beta-dev.1

- kitsune_p2p: Fixes bug where authored data cannot be retrieved locally if the storage arc is not covering that data [\#2425](https://github.com/holochain/holochain/pull/2425)

## 0.3.0-beta-dev.0

- kitsune_p2p: Bump tx5 to include https://github.com/holochain/tx5/pull/31 which should fix the network loop halting on certain error types, like Ban on data send. [\#2315](https://github.com/holochain/holochain/pull/2315)
- kitsune_p2p: Removes the experimental `gossip_single_storage_arc_per_space` tuning param
- kitsune_p2p: Fixes sharded gossip issue where storage arcs are not properly quantized in multi-agent-per-node sharded scenarios. [\#2332](https://github.com/holochain/holochain/pull/2332)
- kitsune_p2p: Add `gossip_arc_clamping` Kitsune tuning param, allowing initial options to set all storage arcs to empty or full. [\#2352](https://github.com/holochain/holochain/pull/2352)
- kitsune_p2p: Changes to arc resizing algorithm to ensure that nodes pick up the slack for freeloading nodes with zero storage arcs. [\#2352](https://github.com/holochain/holochain/pull/2352)
- kitsune_p2p: Disables gossip when using `gossip_arc_clamping = "empty"`: when the arc is clamped to empty, the gossip module doesn’t even activate. [\#2380](https://github.com/holochain/holochain/pull/2380)

## 0.2.0

## 0.2.0-beta-rc.6

## 0.2.0-beta-rc.5

## 0.2.0-beta-rc.4

## 0.2.0-beta-rc.3

## 0.2.0-beta-rc.2

## 0.2.0-beta-rc.1

## 0.2.0-beta-rc.0

- kitsune_p2p: Adds feature flipper `tx5` which enables experimental integration with holochains WebRTC networking backend. This is not enabled by default. [\#1741](https://github.com/holochain/holochain/pull/1741)

## 0.1.0

## 0.1.0-beta-rc.2

## 0.1.0-beta-rc.1

- kitsune_p2p: Fixes some bad logic around leaving spaces, which can cause problems upon rejoining [\#1744](https://github.com/holochain/holochain/pull/1744)
  - When an agent leaves a space, an `AgentInfoSigned` with an empty arc is published before leaving. Previously, this empty-arc agent info was also persisted to the database, but this is inappropriate because upon rejoining, they will start with an empty arc. Now, the agent info is removed from the database altogether upon leaving.

## 0.1.0-beta-rc.0

- kitsune_p2p: **BREAKING CHANGE:** The gossip and publishing algorithms have undergone a significant rework, making this version incompatible with previous versions. Rather than gossiping and publishing entire Ops, only hashes are sent, which the recipient uses to maintain a queue of items which need to be fetched from various other sources on the DHT. This allows for finer-grained control over receiving Ops from multiple sources, and allows each node to manage their own incoming data flow. [\#1662](https://github.com/holochain/holochain/pull/1662)
- kitsune_p2p: **BREAKING CHANGE:** `AppRequest::GossipInfo` is renamed to `AppRequest::NetworkInfo`, and the fields have changed. Since ops are no longer sent during gossip, there is no way to track overall gossip progress over a discrete time interval. There is now only a description of the total number of ops and total number of bytes waiting to be received. As ops are received, these numbers decrement.

## 0.0.52

- kitsune_p2p: The soft maximum gossip batch size has been lowered to 1MB (entries larger than this will just be in a batch alone), and the default timeouts have been increased from 30 seconds to 60 seconds. This is NOT a breaking change, though the usefulness is negated unless the majority of peers are running with the same settings.  [\#1659](https://github.com/holochain/holochain/pull/1659)

## 0.0.51

- kitsune_p2p: `rpc_multi` now only actually makes a single request. This greatly simplifies the code path and eliminates a source of network bandwidth congestion, but removes the redundancy of aggregating the results of multiple peers. [\#1651](https://github.com/holochain/holochain/pull/1651)

## 0.0.50

## 0.0.49

## 0.0.48

## 0.0.47

## 0.0.46

## 0.0.45

## 0.0.44

- kitsune_p2p: Fixes a regression where a node can prematurely end a gossip round if their partner signals that they are done sending data, even if the node itself still has more data to send, which can lead to persistent timeouts between the two nodes. [\#1553](https://github.com/holochain/holochain/pull/1553)

## 0.0.43

- kitsune_p2p: Increases all gossip bandwidth rate limits to 10mbps, up from 0.1mbps, allowing for gossip of larger entries
- kitsune_p2p: Adds `gossip_burst_ratio` to `KitsuneTuningParams`, allowing tuning of bandwidth bursts
- kitsune_p2p: Fixes a bug where a too-large gossip payload could put the rate limiter into an infinite loop

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

- kitsune_p2p: Allow TLS session keylogging via tuning param `danger_tls_keylog` = `env_keylog`, and environment variable `SSLKEYLOGFILE` (See kitsune\_p2p crate api documentation). [\#1261](https://github.com/holochain/holochain/pull/1261)

## 0.0.25

- kitsune_p2p: BREAKING: Gossip messages no longer contain the hash of the ops being gossiped. This is a breaking protocol change.
- kitsune_p2p: Removed the unmaintained “simple-bloom” gossip module in favor of “sharded-gossip”

## 0.0.24

## 0.0.23

- kitsune_p2p: Fixes D-01415 holochain panic on startup [\#1206](https://github.com/holochain/holochain/pull/1206)

## 0.0.22

## 0.0.21

## 0.0.20

## 0.0.19

## 0.0.18

- types: Sharded DHT arcs is on by default. This means that once the network reaches a certain size, it will split into multiple shards.

## 0.0.17

- kitsune_p2p: Agent info is now published as well as gossiped. [\#1115](https://github.com/holochain/holochain/pull/1115)
- kitsune_p2p: BREAKING: Network wire message has changed format so will not be compatible with older versions. [1143](https://github.com/holochain/holochain/pull/1143).
- kitsune_p2p: Fixes to gossip that allows batching of large amounts of data. [1143](https://github.com/holochain/holochain/pull/1143).

## 0.0.16

## 0.0.15

- kitsune_p2p: BREAKING: Wire message `Call` no longer takes `from_agent`. [\#1091](https://github.com/holochain/holochain/pull/1091)

## 0.0.14

## 0.0.13

## 0.0.12

- kitsune_p2p: BREAKING: Return `ShardedGossipWire::Busy` if we are overloaded with incoming gossip. [\#1076](https://github.com/holochain/holochain/pull/1076)
  - This breaks the current network protocol and will not be compatible with other older versions of holochain (no manual action required).

## 0.0.11

- dht_arc: **BREAKING** Arcs are now “unidirectional”, meaning rather than the agent location defining the centerpoint of the storage arc, the agent location defines the left edge of the arc.
  This is a huge change, particularly to gossip behavior. With bidirectional arcs, when peers have roughly equivalently sized arcs, half of the peers who have overlapping arcs will not see each other or gossip with each other because their centerpoints are not contained within each others’ arcs. With unidirectional arcs, this problem is removed at the expense of making peer discovery asymmmetrical, which we have found to have no adverse effects.


## 0.0.10

- kitsune_p2p: Check local agents for basis when doing a RPCMulti call. [\#1009](https://github.com/holochain/holochain/pull/1009).

## 0.0.9

- kitsune_p2p: Fix rpc\_multi bug that caused all request to wait 3 seconds. [\#1009](https://github.com/holochain/holochain/pull/1009/)
- kitsune_p2p: Fix to gossip’s round initiate. We were not timing out a round if there was no response to an initiate message. [\#1014](https://github.com/holochain/holochain/pull/1014).
- kitsune_p2p: Make gossip only initiate with agents that have info that is not expired. [\#1014](https://github.com/holochain/holochain/pull/1014).

## 0.0.8

- dht_arc: New arc resizing algorithm based on `PeerViewBeta`
- dht_arc: In both arc resizing algorithms, instead of aiming for the ideal target arc size, aim for an ideal range. This slack in the system allows all agents to converge on their target more stably, with less oscillation.
- timestamp: **BREAKING**: All chrono logic is behind the `chrono` feature flag which is on by default. If you are using this crate with `no-default-features` you will no longer have access to any chrono related functionality.

### Changed

- kitsune_p2p: `query_gossip_agents`, `query_agent_info_signed`, and `query_agent_info_signed_near_basis` are now unified into a single `query_agents` call in `KitsuneP2pEvent`

## 0.0.7

- types: Adds a prototype protocol for checking consistency in a sharded network.

## 0.0.6

## 0.0.5

## 0.0.4

## 0.0.3

## 0.0.2

## 0.0.1
