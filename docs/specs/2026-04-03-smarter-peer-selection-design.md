# Smarter Peer Selection Using Latency Estimation

**Issue:** #5602
**Date:** 2026-04-03
**Status:** Draft

## Problem

The current peer selection strategy in `holochain_p2p` randomly selects from peers whose storage arc covers the target content. This ignores network latency, leading to slow fetch operations in globally distributed networks.

## Solution Overview

Implement latency-aware peer selection by:

1. Adding Ping/Pong wire messages to measure peer RTT
2. Pinging peers on discovery to build latency estimates
3. Storing latency samples in an in-memory rolling average
4. Replacing random peer selection with weighted random selection favoring lower-latency peers

## Wire Protocol Changes

Add two new variants to `WireMessage` in `crates/holochain_p2p/src/types/wire.rs`:

- `PingReq { msg_id: u64 }` — request, sent to measure RTT
- `PingRes { msg_id: u64 }` — response, sent immediately upon receiving a PingReq

These reuse the existing `msg_id` correlation, oneshot channel, and timeout pattern from `send_request`. No payload is needed.

Upon receiving a `PingReq`, the handler in `handle_space_wire_message_received` sends back a `PingRes` with the matching `msg_id`.

## Latency Store

A new in-memory struct `PeerLatencyStore` in `crates/holochain_p2p/src/peer_latency_store.rs`:

```
PeerLatencyStore {
    estimates: HashMap<Url, LatencyEstimate>,
}

LatencyEstimate {
    samples: VecDeque<Duration>,  // rolling window, max 10
    average: Duration,            // cached rolling average
}
```

### Behaviors

- `record_sample(url, duration)` — pushes a sample, evicts oldest if window is at 10, recalculates average.
- `get_latency(url) -> Option<Duration>` — returns the rolling average, or `None` for unknown peers.
- Thread-safe via `Arc<Mutex<PeerLatencyStore>>`, held on `HolochainP2pActor`.
- No persistence, no TTL. Data lives in memory until node restart.

## Ping Mechanism

### Trigger

When `preflight_validate_incoming` inserts new peer agents into the peer store, a background task is spawned to ping the peer.

### Flow

1. After `space.peer_store().insert(vec![agent])` succeeds, spawn a `tokio::task` per new peer URL.
2. The task sends 10 sequential `PingReq` messages using the existing `send_request` infrastructure (with a 5-second timeout per ping).
3. For each successful `PingRes`, measure elapsed time and call `latency_store.record_sample(url, rtt)`.
4. Failed pings (timeout/error) are skipped. If all 10 fail, the peer has no latency estimate and receives the neutral weight during selection.

Pings are sequential (not parallel) to measure individual RTT rather than congestion. The task runs in the background so the preflight handshake is not blocked. The peer is immediately available for selection at neutral weight while pings run.

## Weighted Peer Selection

Replace the random selection in `get_random_peers_for_location` (`crates/holochain_p2p/src/spawn/actor.rs`).

### Algorithm

1. Get all eligible peers via `get_peers_for_location()` (unchanged — filters by storage arc).
2. Assign weights:
   - Peer with latency estimate: `weight = 1.0 / latency_ms`
   - Peer without estimate: assign the **median weight** of all peers that do have estimates.
   - If no peers have estimates: fall back to uniform random (current behavior).
3. Use `rand::distributions::WeightedIndex` to select `remote_agent_count` peers without replacement.

### Edge Cases

- All peers unknown: uniform random (graceful degradation to current behavior).
- Only one peer available: select it regardless of latency.
- Latency of 0ms: clamp to 1ms to avoid division by zero.

## Testing

### Unit Tests

- `PeerLatencyStore`: recording samples, rolling average calculation, window eviction at 10, `get_latency` returns `None` for unknown peers.
- Weighted selection: lower-latency peers selected more frequently over many iterations; unknown peers get median weight; all-unknown falls back to uniform; 0ms clamping works.

### Integration Tests

- Ping/Pong round-trip: `PingReq` produces `PingRes` with matching `msg_id`.
- End-to-end peer selection: after pinging, `get_random_peers_for_location` prefers lower-latency peers (testable with mock latency store values).

## Files Changed

- `crates/holochain_p2p/src/types/wire.rs` — add PingReq/PingRes variants
- `crates/holochain_p2p/src/spawn/actor.rs` — ping trigger in preflight, weighted selection in get_random_peers_for_location
- `crates/holochain_p2p/src/peer_latency_store.rs` — new file, PeerLatencyStore
- `crates/holochain_p2p/src/lib.rs` — module declaration for peer_latency_store
