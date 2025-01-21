# Holochain Release Log

This file documents results of release tests as described in the [Holochain release process](RELEASE.md).

## 2024-12-17: v0.4.0-rc.2

*Success*
- 3 nodes started a new network, peer discovery succeeded on all nodes within 5 minutes.
- Sending signals to peers succeeded with 100 % reliability.
- Entries created by everyone were received by all peers, mostly instantly. Sometimes it took 60 seconds for the remaining entries to appear.
- Receiving entries that were created in the absence of a node showed a similar pattern. About 50 % of the entries were received immediately after going online, the rest after 60 seconds.
- When a new node is added to an existing network, synchronization of all entries takes about 5 minutes.
- On one occasion one node did not receive the 10 entries published while it was offline. Once the publishing node published another 10 entries, did all 20 come in almost instantly.

## 2025-01-17: v0.4.1-rc.1

*Failure*
- 3 nodes started a new network, peer discovery succeeded on Node A and Node B within 5 minutes. Node C could only see Node A, and no other nodes could see Node C. Logs from Node A indicated that Ops were failing to be integrated due to missing dependencies.
- Sending signals succeeded with 100 % reliability from Node A to Node B, with 50 % reliability from B to A. Node C sent signals with 100% reliability to Node A.
- Entries created by Node A and Node B were received by each other, mostly instantly. Entries created by Node C were not seen by any peers. Entries created by Node A and Node B were not seen by Node C.
- Receiving entries that were created in the absence of a node showed the same pattern.
