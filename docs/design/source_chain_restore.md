# Source Chain Restore Design

## Overview

This document describes a design for restoring an agent's source chain on a fresh Holochain installation by reading the agent's previously authored history back from the DHT. The aim is to let an existing agent key resume participation in a DNA on a new node without re-genesis (which would fork the agent's chain).

Restore covers _public_ chain state only. It does not, and cannot, recover private entries, capability claims, validation receipts, or other purely local state. Agent key recovery itself is out of scope; we assume that an external layer has already made the agent's signing key available to the local Lair keystore instance before install.

It is a largely independent feature design built on top of the existing system; [`data_model.md`](./data_model.md) and [`state_model.md`](./state_model.md) are referenced where their definitions apply. Concretely, the feature introduces **a new cell instantiation workflow** that replaces the genesis workflow on installs flagged as restoring, plus the supporting API and configuration described below.

## Scope and assumptions

In scope:

1. A node that has never previously held this agent's authored data locally (empty per-DNA database, no cached DHT data, no authored rows).
2. The same `AgentPubKey` was previously used on some other node to author a chain in the same DNA, and that chain's public ops are reachable somewhere on the DHT.
3. The agent's signing key is present in the local Lair keystore at install time (otherwise no future action could be signed even after restore, so the install is not useful).
4. Both full-arc and zero-arc installations.

Out of scope:

1. Recovery of the agent signing key itself. The layer that installs the app is responsible for provisioning the key into Lair.
2. Recovery of private entries (kept in `PrivateEntry`; never distributed).
3. Recovery of `CapClaim` rows (never on the DHT).
4. Recovery of any local-only state such as `ChainLock`, scratch state, or received validation receipts.
5. Multi-cell coordination (each cell restores its own chain independently).

## Background

### Installing with an existing agent key

Holochain already supports installing an app with a caller-supplied agent key via the `agent_key: Option<AgentPubKey>` field on [`InstallAppPayload`](../../crates/holochain_types/src/app.rs). When `Some(key)` is provided the conductor uses that key as the cell's author; when `None` it generates a fresh key.

The current install path performs no Lair-presence check at admit time; signing failures surface much later when genesis attempts to produce its first signature. For the restore path this is too late — the workflow needs to know up front that signing will work. This design therefore adds a Lair-presence check at install admit time when `restore_from_dht` is set, together with a test that the check rejects installs whose `agent_key` is not present in the local Lair keystore.

For the purposes of this design we treat "install with existing agent key" as a precondition that already works, with the addition of the admit-time check above.

### Why genesis blocks naive restore

Cell genesis is run unconditionally on any empty per-DNA database. The only short-circuit is `GenesisWorkspace::has_genesis`, which counts existing `Action` rows for the author and returns early once there are three or more. With an empty database that check returns `false`, and the workflow proceeds to write fresh `Dna`, `AgentValidationPkg`, and `Create(Agent)` actions at sequence numbers 0, 1, and 2, signed by the agent now and stamped with the current `Timestamp`.

The agent's previous chain on the DHT already has actions at sequence 0, 1, and 2 with different timestamps and different action hashes. Publishing a second set of seq 0–2 actions for the same author is, by definition, a chain fork. The agent's activity authorities would observe two different actions claiming the same sequence position, raise `ChainIntegrityWarrant::ChainFork`, and the agent would be warranted out of the network on first publish.

Restore therefore cannot reuse the existing install path unmodified. The seq 0–2 actions on the restored chain **must** be the original ones fetched from the DHT, not freshly authored locally.

### Authored vs DHT data in the per-DNA database

Per [`state_model.md`](./state_model.md), each DNA cell uses a single database holding authored chain rows, integrated DHT ops, validation limbo, and cached data. Self-authored ops are inserted with `record_validity = 1` (pre-validated) and bypass the validation limbo. Network-received ops land in `LimboChainOp` first and are promoted to `ChainOp` only after sys and app validation succeed.

There is no existing reverse path that takes DHT-side ops authored by us-on-another-node and reinstates them as authored rows on this node. Restore introduces that path. Because validation rules are immutable for a given DNA — a record signed by `A` that was previously valid remains valid forever under the same rules — restore does not re-run sys or app validation on incoming records; the signature on each `Record` is the trust anchor. The detailed verification model is set out in Step 1 below.

### `get_agent_activity` as the primary retrieval mechanism

The DHT already indexes every action by its author via the `AgentActivity` op, whose authority is the set of peers near the agent's pubkey location. The `get_agent_activity` query exists end-to-end: HDK call, ribosome host function, cascade orchestration, p2p fan-out, authority-side SQL. Its options are defined on [`GetActivityOptions`](../../crates/holochain_p2p/src/types/actor.rs).

When called with `include_full_records: true` it returns complete `Record`s — each action together with its public entry where applicable — so restore can use a single call type to retrieve both action and entry data for the whole chain. Private entries are not returned (they are never distributed). The response also carries `ChainStatus` (Empty / Valid / Forked / Invalid with a `ChainHead`), `HighestObserved` (highest observed sequence plus candidate hashes at that sequence), and any `SignedWarrant`s the authority holds for the agent.

The current implementation's p2p fan-out (`HolochainP2pActor::get_agent_activity`) selects several peers near the agent's location and races them through `select_ok_non_empty`, returning the **first** non-empty response without aggregating across peers. That suffices for application-level reads but not for restore, which needs each authority's view independently in order to require agreement. This is addressed in the API extension section below.

### Arc behaviour and why it matters

Per [`state_model.md`](./state_model.md), a cell's storage arc determines which DHT ops it stores locally. Self-authored ops are also conditionally copied into the DHT side of the database, gated by `arc.contains(loc)` (see `authored_ops_to_dht_db`). For a full-arc node, the agent's own pubkey location falls inside its arc, so its `AgentActivity` ops naturally appear on the DHT side as well. For a zero-arc node, the agent stores none of its own ops on the DHT side; it relies on remote authorities for any DHT-side read.

For restore, this means:

- A full-arc node could in principle restore by joining the DHT, waiting for normal gossip to deliver ops for its own pubkey location, and then re-deriving authored rows from those ops. This works but is implicit and timing-dependent.
- A zero-arc node receives no such gossip, so it cannot reconstruct the chain passively.

To support both arcs uniformly and avoid race conditions where the agent starts authoring before its arc is filled, restore should be **active**: explicitly fetch the chain via `get_agent_activity` (and follow-up record fetches if needed) rather than waiting on gossip.

## Problem statement

Given:

- An empty per-DNA database on this node.
- An `AgentPubKey` `A` that previously authored a chain on this DNA elsewhere.
- The agent's signing key for `A` present in the local Lair keystore.
- An arbitrary local storage arc (full, partial, or zero).

The node does not need to be online with peers reachable at the moment install is requested. If no peers can be reached, the restore workflow waits and retries; the cell remains installed but in a non-callable "restoring" state until restore succeeds or hits a permanent failure.

Produce, atomically before the cell is enabled for application use:

1. A complete authored source chain in the per-DNA database for `A`, with the original action hashes and signatures.
2. A guarantee that the restored chain is a single linear chain from seq 0 to some head seq `H`, with no gaps and no forks.
3. A guarantee that no fresh genesis was written and that nothing new has been published to the DHT for `A` as a side effect of install. (Republishing of restored ops to gather validation receipts is a separate, intentional output described below; it is not new authoring.)

Failure modes that must be explicit (no silent corruption):

- The DHT reports a fork or any warrant for `A`. Restore aborts permanently; the chain is unrecoverable.
- An action received from the DHT fails its signature check against `A`. Restore aborts permanently.
- Authorities are unreachable, return partial chains, or disagree on the chain head. Restore does not abort: the workflow keeps retrying until conditions stabilise.

## Design

### High-level shape

Restore is a new cell instantiation workflow that replaces `genesis_workflow` for installs flagged as restoring. It runs against the same empty per-DNA database that genesis would have run against, but the database it produces is one in which the authored chain has been reconstructed from the DHT instead of freshly created.

The cell stays in a non-callable "restoring" state for the duration of the workflow. Zome calls — including any that would author new chain actions — are rejected while in this state. Network activity that supports the restore itself proceeds normally: incoming gossip, op validation, and integration must run so that records fetched as part of restore can land in the DHT side of the database, so writes performed by those workflows are not blocked by the cell state. What is blocked is application-driven chain authoring.

### Install API extension

Extend `InstallAppPayload` with an opt-in restore flag:

```rust
pub struct InstallAppPayload {
    pub source: AppBundleSource,
    pub agent_key: Option<AgentPubKey>,
    // ...
    /// If true, suppress genesis for all cells in this app and instead reconstruct
    /// each cell's source chain by fetching the agent's prior chain from the DHT.
    /// Requires `agent_key` to be Some.
    #[serde(default)]
    pub restore_from_dht: bool,
}
```

There is no `ignore_restore_failure` analogue to `ignore_genesis_failure`: the existing flag has caused operational confusion (cells left in undefined post-failure states) and is not reproduced here. Restore failures either resolve themselves by retry (transient network conditions) or are permanent (fork, warrant, signature mismatch), and the cell state reflects whichever holds.

Rationale for a flag rather than a separate admin call: install already accepts an existing agent key, runs per-cell genesis, and handles partial-failure cleanup. Restore is a per-cell variant of the same instantiation sequence. A new admin variant would duplicate cell wiring, role settings, membrane-proof handling, and uninstall-on-failure logic.

Validation at install entry:

- `restore_from_dht == true` and `agent_key == None` is rejected (there is nothing to restore for an unknown key).
- The conductor checks that `agent_key` is present in the local Lair keystore and rejects the install at admit time if it is not. The check is exercised by an install test that asserts the rejection path.

### Restore workflow

Per cell, after install has constructed the cell scaffolding but before genesis would have run. The workflow has three steps.

#### Step 1 — Pin the target via `get_agent_activity_multi`

On every fresh start of the workflow, unconditionally issue a `get_agent_activity_multi` call (see [Required p2p API extension](#required-p2p-api-extension) below). The workflow does not consult any persisted result from a previous run; reasons are covered in [Crash recovery](#crash-recovery) below.

Per-peer request:

```rust
GetActivityOptions {
    include_valid_activity: true,
    include_rejected_activity: true,
    include_warrants: true,
    include_full_records: true,   // we want full Records, not just hashes
    // ...
}
```

Aggregation rules across the per-peer responses:

- If any peer reports `ChainStatus::Forked`, `ChainStatus::Invalid`, or returns a `SignedWarrant` whose inner warrant is a `ChainIntegrityWarrant` for `A`, abort permanently. The chain is unrecoverable for this agent. Other warrant variants (none exist today, but the surface is reserved) are not treated as unconditionally fatal here.
- The workflow targets unanimous agreement on `ChainHead` across all queried peers. If fewer than the configured quorum (see [Configuration](#configuration)) responses arrive, or if the responses that do arrive do not all report the same `(seq, hash)` chain head, the workflow does not abort. It waits and retries Step 1 from scratch (see [Retry behaviour](#retry-behaviour)).
- When all queried peers agree on the same `(H, head_hash)`, that pair is the **target head**.

The records returned in the responses are not handed off to any pre-existing validation pipeline. Validation is deterministic and already covered by the signature: a record signed by `A` that was previously valid remains valid by the same rules forever. Restore performs a signature check on each returned `Record` against `A`'s public key and trusts the result. Any record whose signature does not verify against `A` aborts restore permanently — the responding authority is either misbehaving or feeding fabricated content.

#### Step 2 — Write the authored state

The aim of Step 2 is to recreate the authored state that would otherwise be missing on this node. There is no separate "copy after integration" pass: instead, as records arrive from `get_agent_activity_multi` and pass their signature check, the workflow writes them directly into the per-DNA database in the same shape that authoring would have produced, modulo the missing private data.

For each signature-verified record `(action, entry?)` belonging to the target chain, the workflow writes:

- The `Action` row (referenced by both authored-chain queries and DHT op rows).
- The public `Entry` row, where the record carries one. Private entries are not present in the response and are simply absent on the restored node.
- The full set of `ChainOp` rows that the action would produce per [`data_model.md`](./data_model.md) (e.g. `AgentActivity`, `CreateRecord`, plus type-specific ops). These are flagged as accepted by virtue of being part of an authored chain we trust by signature; they do not pass through the limbo or validation tables.
- `ChainOpPublish` entries for each generated op. **Republishing is intentional.** Although the ops already exist on the DHT, the restoring node needs to gather its own validation receipts to reconstruct that portion of local state, and the publish path is the mechanism by which receipts are collected.
- Auxiliary index rows that the action type requires (e.g. `CapGrant` rows for `CapGrant`-typed entries whose entry body is private but whose action is present).

The exact set of tables written to is governed by the data model and state model documents and may need to be revisited if those documents change.

The workflow proceeds as records arrive; ordering is not required because each record is self-validating via its signature and the chain's hash linkage is established by the action contents themselves, not by write order. The watch ends when every sequence number in `0..=H` has been written and the action hash at each sequence number matches the candidate from Step 1's quorum response.

If a target-chain action fails its signature check, or if a record claimed to be in the target chain hashes to a value other than the agreed candidate for its sequence, restore aborts permanently.

#### Step 3 — Activate the cell

When the chain is complete (every sequence `0..=H` is written and matches the pinned hashes), the cell transitions out of the "restoring" state and is enabled for zome calls. New actions authored after this point publish through the normal publish workflow.

The cell's status is exposed via the existing app API surface so that an application can observe that a cell is in "restoring" state and is not yet callable. At the transition to enabled, a new system signal is emitted so that subscribing apps learn about the completion without polling.

#### Retry behaviour

Failures classified as retryable in this design are handled inside the workflow: the cell stays installed, the workflow waits with backoff, and re-runs Step 1 when conditions change. The conductor does not surface a "retry restore" admin call. The retryable category covers:

- Zero peers respond.
- Fewer than `restore_chain_quorum` peers respond.
- Responding peers disagree on `ChainHead`.

Permanent failures (fork, warrant, signature failure, hash mismatch) bypass retry and uninstall the cell with an explicit error.

### Crash recovery

Holochain workflows already retain enough state to recover across conductor restarts. A crash during restore leaves the per-cell "restoring" state and any rows written so far in a consistent state. On restart, the workflow re-enters Step 1 unconditionally (re-running `get_agent_activity_multi` from scratch), so no special checkpoint or "restore-in-progress" marker is introduced specifically for this workflow.

Re-running Step 1 on resume is intentional. The network's view of the agent's chain may have changed between the original Step 1 call and the resume — peers come and go, gossip propagates — and the safe default is to re-query from scratch rather than rely on a possibly out-of-date pinned head. This is _not_ primarily about the author writing more chain content elsewhere in the meantime: an agent that uses restore should not also be authoring on another node, and if they do they are creating a fork themselves. The retry-from-scratch design treats that scenario as an extra safety margin, but the substantive reason is network view drift.

> **Caution:** Using restore shortly after authoring on the original node is high risk. Newly authored ops may not have propagated to all activity authorities yet. A restore that pins a stale head and activates the cell can then have the agent author on top of that stale tip, producing a fork as the original chain's later actions reach the network. Operators should wait long enough for the original node's most recent ops to have visibly propagated before triggering restore.

### Configuration

Quorum is exposed as a new optional conductor configuration parameter (not part of `InstallAppPayload`, because it applies to every restore the conductor performs):

```rust
pub struct ConductorConfig {
    // ...
    /// Number of peers that must agree on the chain head during source-chain
    /// restore for the restore to succeed.
    ///
    /// Increasing this value raises the cost of feeding a restoring agent a
    /// fabricated or partial history but also raises the chance that restore
    /// fails on small or poorly-connected DHTs.
    #[serde(default = "default_restore_chain_quorum")]
    pub restore_chain_quorum: u8,
}

fn default_restore_chain_quorum() -> u8 {
    // chosen so that restore works on small networks while still requiring
    // independent agreement from more than one authority
    2
}
```

The `get_agent_activity_multi` call fans out to a peer count derived from this quorum (e.g. `quorum + 1` to allow for slow or unresponsive peers without abandoning a recoverable restore). The exact fan-out factor is an implementation detail of the p2p layer; only the quorum is operator-visible.

### Init handling

The genesis workflow normally precedes any init zomes callback, and `InitZomesComplete` is appended to the chain by the first zome call after genesis succeeds. Init runs at most once, gated by the presence of `InitZomesComplete` on chain.

Restore is well-behaved with respect to init by construction:

- If the prior chain on the DHT contains `InitZomesComplete`, that action is restored as part of the chain. Subsequent zome calls observe it and skip init.
- If the prior chain does not contain `InitZomesComplete` (the agent was installed but never made a zome call between genesis and the install on the original node), restore brings back only the genesis prefix. The next zome call after activation runs init normally and appends a fresh `InitZomesComplete` on top of the restored tip. This is correct because the chain on the DHT is well-defined and we are simply extending it.

### Treatment of forks and warrants

If any peer in Step 1 reports a fork (`ChainStatus::Forked` or a `ChainIntegrityWarrant::ChainFork` in `warrants`), the chain is unrecoverable: by definition the agent has already misbehaved on the DHT, and restoring either branch and continuing to author would compound the fork. Restore aborts permanently with an error that explicitly identifies the conflicting actions, so the operator can decide whether to abandon the agent key.

If any peer reports `ChainStatus::Invalid` or returns an `InvalidChainOp` warrant, restore likewise aborts permanently. The agent has prior invalid behaviour on record; new authoring would inherit the warranted state.

### Partial-arc and zero-arc considerations

Step 1's `get_agent_activity_multi` call routes via the cascade and p2p layers, which already handle the local-vs-remote distinction. It works without modification on a zero-arc node, because the cascade routes to remote authorities for anything not held locally.

For full-arc nodes, gossip will eventually deliver the agent's own ops over the normal sync path as well. Restore deliberately does **not** wait for that — it actively fetches via `get_agent_activity` — because waiting for gossip introduces uncontrolled latency and lacks an explicit termination criterion.

### Completeness termination

Step 2 cannot rely solely on the target head being present, because records arriving from a multi-peer fan-out are not guaranteed to be written in sequence order. The workflow terminates only when every sequence number in `0..=H` is present in the authored state and the action hash at each sequence matches the candidate pinned by Step 1. While any sequence is missing the workflow continues to accept and write incoming records, re-requesting any sequences still absent after a tunable wait.

The agreement-based quorum in Step 1 is the only knob that defends against a single misbehaving authority feeding the restoring node a fabricated view; see [Configuration](#configuration).

## Required p2p API extension

The current `HolochainP2pActor::get_agent_activity` (`crates/holochain_p2p/src/spawn/actor.rs:2062`) fans out to a weighted set of peers but collapses to the first non-empty response via `select_ok_non_empty`. Restore needs the raw per-peer responses so that the quorum logic above is possible.

Add a new p2p call `get_agent_activity_multi(target_peer_count, min_responses, timeout) -> Vec<AgentActivityResponse>` that returns all responses received within the timeout, up to the target count, paired with the peer that produced each. The existing `get_agent_activity` is left unchanged so application-level callers continue to get the cheap first-non-empty behaviour.

The authority-side handler (`crates/holochain_cascade/src/authority.rs:54`) needs no changes; it already returns whatever the authority holds.

## What is explicitly not restored

| Data | Storage | Why not restored |
|------|---------|------------------|
| Private entries | `PrivateEntry` table | Never distributed on DHT; only the authoring node ever held them. |
| Cap claims | `CapClaim` table | Local-only index of grants received from other agents; not chain data. |
| Cap grants (entry body) | Action restored, entry body not | `CapGrant` entries are private; the action restores but the entry body cannot be retrieved. The agent loses the ability to exercise prior grants. |
| Received validation receipts | Validation receipt store | Receipts other peers issued for this agent's authored ops; not part of the chain itself. These are rebuilt over time after restore by republishing — see Step 2. |
| Pending counter-signing sessions | `ChainLock` | Transient. Any session in flight on the original node is lost. |
| Scratch state | in-memory | Transient by definition. |

Applications that rely on the lost categories must accept degraded post-restore behaviour, or the layer that triggers restore must arrange an out-of-band channel for them. This design does not attempt to address either.

## Failure modes and operator behaviour

| Failure | Behaviour |
|---------|-----------|
| `agent_key == None` with `restore_from_dht == true` | Reject the install at admit time. |
| `agent_key` not present in local Lair | Reject the install at admit time. |
| Fewer than `restore_chain_quorum` peers respond | Internal retry (see [Retry behaviour](#retry-behaviour)). Cell stays installed in "restoring" state. |
| Responding peers disagree on `ChainHead` | Internal retry. Cell stays installed in "restoring" state. |
| Fork reported by any peer | Permanent failure. Chain is unrecoverable for this key. |
| `ChainIntegrityWarrant` returned for the agent | Permanent failure. (Other warrant variants in future may be handled differently; only chain integrity warrants are unconditionally fatal for restore.) |
| Restored record fails its signature check against `A` | Permanent failure. |
| Restored record's hash does not match the Step 1 candidate for its sequence | Permanent failure. |
| Crash mid-restore | Workflow re-enters Step 1 on next conductor start (see [Crash recovery](#crash-recovery)); previously written authored-state rows are reused. |

