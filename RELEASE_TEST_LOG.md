# Holochain Release Log

This file documents results of release tests as described in the [Holochain release process](RELEASE.md).

## 2025-08-05: v0.5.5-rc.2

*Success*
- Ran a test with 3 nodes.
- Peer discovery was fast.
- For nodes joining a network with existing data, the initial sync took around a minute.
- Signals were sent with 100% reliability.
- Entries created by all nodes were received by all peers, mostly instantly.
- A node going offline and then coming back online received all entries created while it was offline, taking a little 
  over 30 seconds to sync.
- Started a 4th node and waited for it to sync with the network. It took a few minutes to sync which is a little slower
  than when the network had less test data, but still acceptable.

## 2025-08-05: v0.5.5-rc.0

*Failure*
- Ran a test where two nodes were started and created data, then two other nodes joined later.
  - All test steps were successful for signals and entry creation.
  - However, the users who joined later were not able to reach full arc. Data was synced through gossip but not publish
    due to the lack of full arc.
  - This was a different test to what we normally do because we wanted to verify a fix to the data syncing issues that
    should have been fixed in this release.

- Ran a second test where all 4 nodes started with a new network seed at the same time.
- Initially, all checks passed and everyone was able to reach full arc quickly.
- After one node was shut down, the other three nodes were able to create data and sync it with each other.
- When the offline node was brought back online, it was able to sync all data created while it was offline. However, it
  was unable to declare full arc and appeared to cease gossiping.
- Upon restarting the offline node, having synced all data, it was able to quickly declare full arc.

## 2025-07-09: v0.4.4-rc.0

*Success*
- A quick smoke test with 7 nodes.
- The only changes in this release are to zome call atomicity and the `hc-sandbox` CLI tool. We checked that peer 
  discovery is working, that signals can be sent and that entries created by all nodes are received by all other nodes.

## 2025-07-09: v0.5.4-rc.0

*Success*
- Ran a test with dino-adventure.
- Test with 7 nodes and observed:
  - Peer discovery and initial sync was fast.
  - Sending signals was 100% reliable.
  - Entries created by all nodes were initially received quickly.
  - After taking one node offline and then creating data, some people were able to see created data quickly and for
    other it took some time. In the region of a couple of minutes. This is acceptable while we discover that a peer
    is offline but it then appeared to remain slower after we should have stopped talking to the offline node.
  - Bringing the offline node back online resulted in a nearly immediate sync for that node. When everyone created data
    with the offline node back online, we continued to observe slightly slower sync times than we saw initially with
    everyone online. For most people, data came through quickly. For a few people, it took a couple of minutes.

This doesn't appear to be regression from the previous release, but might suggest that our connection handling once 
peers start to come and go, still needs work.

## 2025-06-25: v0.5.3-rc.0

*Success*
- Ran a test with dino-adventure for the first time.
- Ran a test with 4 nodes and observed:
  - Fast peer discovery.
  - Fast initial sync, with "Full" arc showing within the first 30 seconds.
  - Signals were sent with 100% reliability.
  - Entries created by all nodes were received by all peers, mostly instantly.
  - A node going offline and then coming back online received all entries created while it was offline, within 10 seconds.
- Started a 5th node and then took two people and the spare node offline to confirm:
  - Offline nodes are marked unreachable and Holochain stops contacting them.
  - Online nodes are able to create and sync data, and send signals to each other, without interruption caused by Holochain
    trying to contact the offline nodes.
- Started two instances on a different network and confirmed that with a small network of 2 peers that the initial sync is
  fast for both peers.

## 2025-05-08: v0.5.2-rc.2

*Success*
- Ran a test with 4 nodes. Passed all tests.
- We did see one WebRTC connection timeout that fell back to SBD relay, but after a conductor restart, those peers
  were able to connect over WebRTC.
- We also did some testing with multiple agent infos on the bootstrap server that we then took offline. This didn't 
  behave as well. We were still able to sync data but saw a lot more attempts to connect to, and get data from, the 
  offline nodes than expected. This is not a new problem with this release and has existed since previous releases of 
  Holochain. It is something that we need to follow up on though.

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
