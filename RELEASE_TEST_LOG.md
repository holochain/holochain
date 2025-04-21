# Holochain Release Log

This file documents results of release tests as described in the [Holochain release process](RELEASE.md).

## 2025-04-21: v0.5.0-rc.4

*Success*
- Ran a test with 5 nodes. Peer discovery was quick, then it took a few minutes for the nodes to sync their profiles.
  It appeared that nodes that ended up using SBD instead of WebRTC took a little longer to get going. Once the first
  few nodes were connected, all other checks passed.
- This was the fastest we've seen entries sync and new nodes see content on joining the network.

## 2025-03-10: v0.4.2-rc.1

*Success*
- Day 1: Started with 7 nodes and ran through the first test steps of sending signals, creating entries and syncing 
  with a node that has been offline.
- As this is just a regression test with small changes, we concluded the test there.

## 2025-01-27: v0.4.1-rc.2

*Success*
- Day 1: Started with 6 nodes and ran through all test steps successfully. All peer discovery and op syncing was 
  happening within expected timeframes. All steps were completed within a 30 minute call.
- Day 2: All nodes were able to sync the data created after they went offline on day 1. We lost one original node and 
  had a new node join the testing. So a slight deviation from the test script, but otherwise all checks passed.
- Bootstrapping and initial gossip seemed to take a while to start syncing data and then after a few minutes, 
  everything was showing up as expected. Not unusual behavior for Holochain and the main thing is tha data was
  consistently showing up after a few minutes.

## 2025-01-17: v0.4.1-rc.1

*Failure*
- 3 nodes started a new network, peer discovery succeeded on Node A and Node B within 5 minutes. Node C could only see Node A, and no other nodes could see Node C. Logs from Node A indicated that Ops were failing to be integrated due to missing dependencies.
- Sending signals succeeded with 100 % reliability from Node A to Node B, with 50 % reliability from B to A. Node C sent signals with 100% reliability to Node A.
- Entries created by Node A and Node B were received by each other, mostly instantly. Entries created by Node C were not seen by any peers. Entries created by Node A and Node B were not seen by Node C.
- Receiving entries that were created in the absence of a node showed the same pattern.

## 2024-12-17: v0.4.0-rc.2

*Success*
- 3 nodes started a new network, peer discovery succeeded on all nodes within 5 minutes.
- Sending signals to peers succeeded with 100 % reliability.
- Entries created by everyone were received by all peers, mostly instantly. Sometimes it took 60 seconds for the remaining entries to appear.
- Receiving entries that were created in the absence of a node showed a similar pattern. About 50 % of the entries were received immediately after going online, the rest after 60 seconds.
- When a new node is added to an existing network, synchronization of all entries takes about 5 minutes.
- On one occasion one node did not receive the 10 entries published while it was offline. Once the publishing node published another 10 entries, did all 20 come in almost instantly.
