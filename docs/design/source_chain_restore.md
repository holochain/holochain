# Source Chain Restore Design

## Overview

This document describes a design for restoring an agent's source chain on a fresh Holochain installation by reading the agent's previously authored history back from the DHT. The aim is to let an existing agent key resume participation in a DNA on a new node.

Restore covers _public_ chain state only. It does not, and cannot, recover private entries, capability claims, validation receipts, or other purely local state. Agent key recovery itself is out of scope; it is assumed that an external layer has already made the agent's signing key available to the local Lair keystore instance before install.

It is a largely independent feature design built on top of the existing system; [`data_model.md`](./data_model.md) and [`state_model.md`](./state_model.md) are referenced where their definitions apply. Concretely, the feature introduces **a new cell instantiation workflow** that replaces the genesis workflow on installs flagged as restoring, **a per-app orchestrator** that drives that workflow across all of an app's cells, plus the supporting API and configuration described below.

## Scope and assumptions

In scope:

1. The typical case: a node that has never previously held this agent's authored data locally (empty per-DNA database, no cached DHT data, no authored rows). Restore is also safe to run against a partially-populated database — the signature and hash checks in Step 2 make redundant writes idempotent — but an empty database is the canonical input.
2. The same `AgentPubKey` was previously used on some other node to author a chain in the same DNA, and that chain's public ops are reachable somewhere on the DHT.
3. The agent's signing key is present in the local Lair keystore at install time (otherwise no future action could be signed even after restore, so the install is not useful).
4. Both full-arc and zero-arc installations.
5. Apps with more than one cell. A per-app orchestrator restores each cell's chain in turn; the app reaches a callable state only once every cell has restored. See [Per-app orchestration](#per-app-orchestration).

Out of scope:

1. Recovery of the agent signing key itself. The layer that installs the app is responsible for provisioning the key into Lair.
2. Recovery of private entries (kept in `PrivateEntry`; never distributed).
3. Recovery of `CapClaim` rows (never on the DHT).
4. Recovery of any local-only state such as `ChainLock` or received validation receipts.

## Background

### Installing with an existing agent key

Holochain already supports installing an app with a caller-supplied agent key via the `agent_key: Option<AgentPubKey>` field on [`InstallAppPayload`](../../crates/holochain_types/src/app.rs). When `Some(key)` is provided the conductor uses that key as the cell's author; when `None` it generates a fresh key.

The current install path performs no Lair-presence check at admit time; signing failures surface when genesis attempts to produce its first signature. For the restore path this is too late — the workflow needs to know up front that signing will work. This design therefore adds a Lair-presence check at install admit time whenever `agent_key` is supplied, regardless of whether `restore_from_dht` is set. Applying the check unconditionally means a fresh-install with a caller-supplied key also fails fast on a missing Lair entry instead of dying later in genesis. A test asserts the rejection path for both the restore and the plain `Some(agent_key)` install cases.

For the purposes of this design, "install with existing agent key" is treated as a precondition that already works, with the addition of the admit-time check above.

### Why genesis blocks naive restore

Cell genesis runs unconditionally on an empty per-DNA database (the `GenesisWorkspace::has_genesis` short-circuit needs ≥3 author rows). On a restore install it would write fresh `Dna` / `AgentValidationPkg` / `Create(Agent)` actions at seq 0–2 with current timestamps and therefore different hashes than the originals already on the DHT — a chain fork, raising `ChainIntegrityWarrant::ChainFork` on first publish. The restored seq 0–2 actions must be the originals fetched from the DHT, so restore replaces `genesis_workflow` rather than running before or after it.

### Authored vs DHT data in the per-DNA database

Per [`state_model.md`](./state_model.md), each DNA cell uses a single database holding authored chain rows, integrated DHT ops, validation limbo, and cached data. Self-authored ops are inserted with `record_validity = 1` (pre-validated) and bypass the validation limbo. Network-received ops land in `LimboChainOp` first and are promoted to `ChainOp` only after sys and app validation succeed.

There is no existing reverse path that takes DHT-side ops authored by `A` on another node and reinstates them as authored rows on this node. Restore introduces that path. Because validation rules are immutable for a given DNA — a record signed by `A` that was previously valid remains valid forever under the same rules — restore does not re-run sys or app validation on incoming records; the signature on each `Record` is the trust anchor. The detailed verification model is set out in Step 1 below.

### `get_agent_activity` as the primary retrieval mechanism

The DHT already indexes every action by its author via the `AgentActivity` op, whose authority is the set of peers near the agent's pubkey location. The `get_agent_activity` query exists from cascade orchestration through p2p fan-out to authority-side SQL; the restore workflow calls into the cascade directly rather than going through the HDK/host-function path. Its options are defined on [`GetActivityOptions`](../../crates/holochain_p2p/src/types/actor.rs).

When called with `include_full_records: true` it returns complete `Record`s — each action together with its public entry where applicable — so restore can use a single call type to retrieve both action and entry data for the whole chain. Private entries are not returned (they are never distributed). The response also carries `ChainStatus` (Empty / Valid / Forked / Invalid with a `ChainHead`), `HighestObserved` (highest observed sequence plus candidate hashes at that sequence), and any `SignedWarrant`s the authority holds for the agent.

The current implementation's p2p fan-out (`HolochainP2pActor::get_agent_activity`) selects several peers near the agent's location and races them through `select_ok_non_empty`, returning the **first** non-empty response without aggregating across peers. That suffices for application-level reads but not for restore, which needs each authority's view independently in order to require agreement. This is addressed in the API extension section below.

### Arc behaviour and why it matters

Per [`state_model.md`](./state_model.md) each cell has a single per-DNA database. An agent always stores its own authored content. The cell's storage arc determines which _other_ DHT content it stores locally via `arc.contains(loc)`; for a full-arc node that's everything, for a zero-arc node nothing beyond its own authoring.

For restore, this means:

- A full-arc node could in principle restore by joining the DHT, waiting for normal gossip to deliver ops for its own pubkey location into the local store, and then deriving authored rows from those ops. This works but is implicit and timing-dependent.
- A zero-arc node receives no such gossip, so it cannot reconstruct the chain passively.

To support both arcs uniformly, restore should be **active**: explicitly fetch the chain via `get_agent_activity` (and follow-up record fetches if needed) rather than waiting on gossip.

## Problem statement

Given:

- An empty per-DNA database on this node.
- An `AgentPubKey` `A` that previously authored a chain on this DNA elsewhere.
- The agent's signing key for `A` present in the local Lair keystore.
- An arbitrary local storage arc (full, partial, or zero).

The node does not need to be online with peers reachable at the moment install is requested. If no peers can be reached, the restore workflow waits and retries; the app remains installed in `AppStatus::AwaitingRestore` until restore succeeds or hits a permanent failure.

Produce atomically before the cell is enabled for application use:

1. A complete authored source chain in the per-DNA database for `A`, with the original action hashes and signatures.
2. A guarantee that the restored chain is a single linear chain from seq 0 to some head seq `H`, with no gaps and no forks. `H` is determined by calling `get_agent_activity` against multiple peers and taking the unanimously-agreed `ChainHead` (see [Step 1](#step-1--pin-the-target-via-get_agent_activity_multi)).
3. A guarantee that no fresh genesis was written and that nothing new has been published to the DHT for `A` as a side effect of install. (Republishing of restored ops to gather validation receipts is a separate, intentional output described below; it is not new authoring.)

Failure modes that must be explicit (no silent corruption):

- A locally-validated `ChainIntegrityWarrant` (e.g. `ChainFork`, `InvalidChainOp`) exists for `A`. Restore aborts permanently; the chain is unrecoverable. Remote-reported warrants are not trusted on receipt — see [Treatment of warrants](#step-1--pin-the-target-via-get_agent_activity_multi).
- Authorities are unreachable, return partial chains, or disagree on the chain head. Restore does not abort: the workflow keeps retrying until conditions stabilise.
- Individual records arrive with bad signatures or hashes that do not match the Step 1 candidate. These are discarded silently; restore relies on honest peers to supply a valid record for the same sequence. A misbehaving peer cannot abort restore by serving forgeries because a forged action cannot satisfy the chain's hash linkage and signature check simultaneously, so it can never be written to the authored state. Restore only stalls (and then retries from Step 1) if no honest peer can be reached.

## Design

### High-level shape

Restore is a new cell instantiation workflow that replaces `genesis_workflow` for installs flagged as restoring. It runs against the same empty per-DNA database that genesis would have run against, but the database it produces is one in which the authored chain has been reconstructed from the DHT instead of freshly created.

The app stays in `AppStatus::AwaitingRestore` for the duration of the workflow (new variant, see [App status](#app-status) below). Zome calls — including any that would author new chain actions — are rejected while in this state. Network activity that supports the restore itself proceeds normally: incoming gossip, op validation, and integration must run so that records fetched as part of restore can land in the DHT side of the database, so writes performed by those workflows are not blocked by the app status. What is blocked is application-driven chain authoring.

### Per-app orchestration

An app may comprise more than one cell (one per provisioned role). The unit of restore is the cell — each cell has its own per-DNA database and its own chain to reconstruct — but the unit of status is the app, because Holochain has no per-cell persisted status. A per-app orchestrator bridges the two.

On a restore install the orchestrator:

1. Sets the app to `AppStatus::AwaitingRestore`.
2. Iterates the app's `provisioned_cells()`. Clone cells are not provisioned at install time, so only provisioned cells participate; clones created later go through the normal (non-restore) creation path.
3. Runs the restore workflow for each cell **in sequence, not in parallel**. Restore is signature-check and hash-calculation heavy; serialising cells caps that work at one chain's worth at a time rather than multiplying it across every cell of the app at once. Cells are processed in the deterministic order of the `provisioned_cells()` index map.
4. Emits `SystemSignal::RestoreComplete { cell_id }` as each cell finishes.
5. **Stops at the first cell that fails permanently.** That cell's failure moves the whole app to `AppStatus::Unrecoverable(cell_id, reason)` and emits `SystemSignal::RestoreFailed { cell_id, reason }`; cells later in the order are never attempted. An app missing even one cell cannot run usefully, and the `Unrecoverable` state would block `enable_app` regardless, so there is no value in pressing on. Cells restored before the failure keep their reconstructed chains on disk — that work is not rolled back — but the app will not become callable until the operator resolves the broken cell (in practice, by uninstalling).
6. When every provisioned cell has restored, moves the app to `AppStatus::Disabled(DisabledAppReason::NeverStarted)` and emits `SystemSignal::AppRestoreComplete { installed_app_id }`. As with any install, the app is not enabled automatically; the caller invokes `enable_app` separately.

The remainder of this document describes the per-cell restore workflow that the orchestrator runs. Except where it refers to app-level status transitions, every step is scoped to a single cell.

### App status

Restore introduces two new `AppStatus` variants:

```rust
pub enum AppStatus {
    Enabled,
    Disabled(DisabledAppReason),
    AwaitingMemproofs,
    /// Restore is in progress for one or more of the app's cells. Zome calls
    /// rejected. Transitions to Disabled(NeverStarted) once every cell has
    /// restored, or to Unrecoverable as soon as any one cell fails permanently.
    AwaitingRestore,
    /// Restore hit a permanent failure on one of the app's cells (a
    /// locally-validated chain-integrity warrant against `A`). Terminal: the
    /// app cannot be enabled and must be uninstalled. Names the failing cell.
    Unrecoverable(CellId, UnrecoverableCellReason),
}
```

`AwaitingRestore` is analogous to the existing `AwaitingMemproofs` — installed but not yet callable, with a workflow running to clear the precondition. `Unrecoverable` is new; it expresses an outcome that existing variants cannot capture cleanly (`Disabled` can be re-enabled, but a forked chain cannot become un-forked). The status is app-level: an app with several cells does not get a per-cell persisted status. The failing `CellId` is carried in the `Unrecoverable` variant so the operator knows which cell broke.

`UnrecoverableCellReason` captures the cause for operator visibility, at minimum:

```rust
pub enum UnrecoverableCellReason {
    /// Two or more conflicting actions at the same sequence position on the
    /// agent's chain — proven by a `ChainIntegrityWarrant::ChainFork` that
    /// local validation has accepted.
    ChainFork(WarrantSummary),
    /// Any other validated `ChainIntegrityWarrant` (e.g. `InvalidChainOp`).
    ChainIntegrityWarrant(WarrantSummary),
    // Reserved for future restore failure categories.
}
```

`ChainStatus::Invalid` and `ChainIntegrityWarrant` are not separate failure categories — `ChainStatus::Invalid` is the authority's classification _when it has accepted_ a `ChainIntegrityWarrant`. Restore therefore keys off the warrant (after validating it locally; see [Treatment of warrants](#step-1--pin-the-target-via-get_agent_activity_multi)), not the status flag in isolation. An incomplete or empty response is **not** a failure: it returns the workflow to retry, not to `Unrecoverable`.

State transitions during restore (driven by the per-app orchestrator; see [Per-app orchestration](#per-app-orchestration)):

```text
install_app(restore_from_dht: true)
    └─> AppStatus::AwaitingRestore
            ├─(every cell's chain integrity check passes)─> AppStatus::Disabled(NeverStarted)
            │                                                     └─(enable_app)─> AppStatus::Enabled
            └─(any cell fails permanently)───────────────> AppStatus::Unrecoverable(cell_id, reason)
```

Three new system signals accompany the transitions. The per-cell signals carry `cell_id`; the app-level completion signal carries `installed_app_id`:

- `SystemSignal::RestoreComplete { cell_id }` — emitted as each individual cell finishes restoring.
- `SystemSignal::AppRestoreComplete { installed_app_id }` — emitted once when every cell of the app has restored and the app reaches `Disabled(NeverStarted)`.
- `SystemSignal::RestoreFailed { cell_id, reason: UnrecoverableCellReason }` — emitted when a cell fails permanently; the app moves to `Unrecoverable` at the same moment, so this doubles as the app-level failure notice.

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

There is no `ignore_restore_failure` analogue to `ignore_genesis_failure`. Restore failures either resolve themselves by retry (transient network conditions) or are permanent (fork or warrant), and the app status reflects whichever holds.

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

- The workflow targets unanimous agreement on `ChainHead` across all queried peers. If fewer than the configured quorum (see [Configuration](#configuration)) responses arrive, or if the responses that do arrive do not all report the same `(seq, hash)` chain head, the workflow does not abort. It waits and retries Step 1 from scratch (see [Retry behaviour](#retry-behaviour)).
- When all queried peers agree on the same `(H, head_hash)`, that pair is the **target head**.
- Warrant handling — see below. The presence of a warrant in a response does **not** by itself fail restore; warrants must be validated locally first.

**Treatment of warrants.** Warrants returned by remote peers are never trusted on receipt — they are accusations, not proofs, until local validation has run. When any peer's response includes one or more `SignedWarrant`s naming `A`, the workflow:

1. Submits the warrants to the local validation pipeline as it would any other received warrant.
2. Enters a polling/wait loop, leaving the app in `AppStatus::AwaitingRestore`. The workflow does not advance to Step 2 while warrant validation is outstanding for `A`.
3. Resolves the loop when validation has reached a verdict on every warrant against `A`:
   - If **any** warrant is validated as a `ChainIntegrityWarrant` against `A`, restore transitions the app to `AppStatus::Unrecoverable` with the appropriate reason (`ChainFork` or `ChainIntegrityWarrant`). The chain is unrecoverable for this agent.
   - If **all** warrants are validated as invalid (or rejected by the validation pipeline), the workflow proceeds to use the responses' chain data as if no warrants had been reported.

Other warrant variants beyond `ChainIntegrityWarrant` (none exist today, but the surface is reserved) are not unconditionally fatal here; they fall through to local validation and are handled per the validation pipeline's verdict.

**Signature checks on records.** The records returned in the responses are not handed off to any pre-existing validation pipeline. Validation is deterministic and already covered by the signature: a record signed by `A` that was previously valid remains valid by the same rules forever. Restore performs a signature check on each returned `Record` against `A`'s public key and trusts the result. Records whose signature does not verify against `A` are discarded, not treated as a fatal error — a single misbehaving authority that returns forged actions would otherwise be able to block restore for that agent indefinitely. The discarded records are absent from the authored state; honest peers' responses provide the same `(seq, hash)` slots, so Step 2's completeness check (every seq 0..=H present and matching the quorum hash) is the safety net.

#### Step 2 — Write the authored state

The aim of Step 2 is to recreate the authored state that would otherwise be missing on this node. There is no separate validation and integration flow: instead, as records arrive from `get_agent_activity_multi` and pass their signature check, the workflow writes them directly into the per-DNA database in the same shape that authoring would have produced, modulo the missing private data.

For each signature-verified record `(action, entry?)` belonging to the target chain, the workflow writes:

- The `Action` row (referenced by both authored-chain queries and DHT op rows).
- The public `Entry` row, where the record carries one. Private entries are not present in the response and are simply absent on the restored node.
- The full set of `ChainOp` rows that the action would produce per [`data_model.md`](./data_model.md) (e.g. `AgentActivity`, `CreateRecord`, plus type-specific ops). These are flagged as accepted by virtue of being part of an authored chain trusted by signature; they do not pass through the limbo or validation tables.
- `ChainOpPublish` entries for each generated op. **Republishing is intentional.** Although the ops already exist on the DHT, the restoring node needs to gather its own validation receipts to reconstruct that portion of local state, and the publish path is the mechanism by which receipts are collected.
- Auxiliary index rows that the action type requires (e.g. `CapGrant` rows for `CapGrant`-typed entries whose entry body is private but whose action is present).

The exact set of tables written to is governed by the data model and state model documents and may need to be revisited if those documents change.

The workflow proceeds as records arrive; ordering is not required because each record is self-validating via its signature and the chain's hash linkage is established by the action contents themselves, not by write order. The watch ends when the chain-integrity check (below) succeeds.

Records claimed to be in the target chain that fail their signature check, or that hash to a value other than the quorum-agreed candidate for their sequence, are discarded. The discarded records are never written to the authored state. Restore relies on at least one honest peer per sequence supplying a record that passes both checks; until that holds for every `seq` in `0..=H`, the workflow keeps requesting and waits.

**Chain-integrity check (gate to Step 3).** Before transitioning out of restoring, the workflow verifies that the authored state contains a single linear chain for `A`:

- Every sequence number in `0..=H` is present, no gaps.
- The action hash written at each sequence matches the candidate hash pinned by Step 1's quorum response.
- The action at seq `H` has hash equal to the `head_hash` agreed by all queried peers in Step 1 (terminates the check on the agreed tip, not just any tip).
- Each action at seq `n > 0` has `prev_action` equal to the action hash at seq `n - 1` (sanity check; the hashes already encode this, but the load step verifies it explicitly).

This check loads the chain from the local store. Any failure means restore is not yet complete and the workflow returns to waiting/requesting. The check is the implementation of the guarantee in [Problem statement](#problem-statement) that a complete, gap-free, fork-free chain is in place before the cell may be used.

#### Step 3 — Hand the cell back to the orchestrator

When the chain-integrity check succeeds for this cell, the workflow reports completion to the per-app orchestrator and a `SystemSignal::RestoreComplete { cell_id }` is emitted so subscribers learn this cell is done without polling. The cell's reconstructed chain is now on disk; nothing further happens to it until the app is enabled.

App-level status does **not** change on a single cell completing. The orchestrator advances to the next provisioned cell (see [Per-app orchestration](#per-app-orchestration)). Only once **every** cell of the app has reached this point does the app transition out of `AppStatus::AwaitingRestore` into `AppStatus::Disabled(DisabledAppReason::NeverStarted)` — the same state a non-restoring install lands in after genesis — and emit `SystemSignal::AppRestoreComplete { installed_app_id }`. The app is **not** enabled automatically; the caller must invoke `enable_app`, matching existing install semantics where installation and activation are separate admin operations. After `enable_app`, new actions authored on this node publish through the normal publish workflow on top of each cell's restored tip.

If this cell reaches a permanent failure (see [Failure modes](#failure-modes-and-operator-behaviour)), the orchestrator moves the whole app to `AppStatus::Unrecoverable(cell_id, reason)` and emits `SystemSignal::RestoreFailed { cell_id, reason }`, then stops without attempting any remaining cells. `Unrecoverable` is a terminal state: the app cannot be enabled and must be uninstalled. The signal carries enough detail (fork, warrant variant, failing cell) for an operator UI to explain the outcome.

#### Retry behaviour

Failures classified as retryable in this design are handled inside the workflow: the app stays in `AppStatus::AwaitingRestore`, the workflow waits with backoff, and re-runs Step 1 when conditions change. The conductor does not surface a "retry restore" admin call. The retryable category covers:

- Zero peers respond.
- Fewer than `restore_chain_quorum` peers respond.
- Responding peers disagree on `ChainHead`.
- Incomplete chain returned (gaps in `0..=H`): keep requesting missing sequences from peers that might hold them.
- Warrants returned by peers are pending local validation: wait for the validation pipeline's verdict before advancing.
- Individual records arrive with bad signatures or hashes that do not match the Step 1 candidate. These are discarded record-by-record; restore keeps requesting until at least one honest peer per `seq` has been heard from.

Permanent failures (any locally-validated `ChainIntegrityWarrant` against `A`) bypass retry and transition the app to `AppStatus::Unrecoverable`, emitting `SystemSignal::RestoreFailed`. The app is **not** automatically uninstalled — the operator decides whether to uninstall after inspecting the failure reason. The terminal `Unrecoverable` state prevents `enable_app`, so no further authoring can occur on a chain known to be unrecoverable.

### Crash recovery

Holochain workflows already retain enough state to recover across conductor restarts. A crash during restore leaves the app in `AppStatus::AwaitingRestore` with any rows written so far in a consistent state. On restart, the workflow re-enters Step 1 unconditionally (re-running `get_agent_activity_multi` from scratch), so no special checkpoint or "restore-in-progress" marker is introduced specifically for this workflow.

The per-app orchestrator resumes the same way without any persisted per-cell progress flag. On restart it finds the app in `AppStatus::AwaitingRestore` and walks `provisioned_cells()` from the start of the deterministic order, re-running the restore workflow for each cell. There is no separate "is this cell already done?" lookup: a cell restored before the crash simply re-runs its workflow, which re-pins the head in Step 1 and then satisfies the Step 2 chain-integrity check immediately because the chain is already present. Step 2 writes are idempotent (signature and hash checks make redundant writes no-ops), so re-running an already-restored or partially-restored cell is safe and converges without duplicating data.

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
- If the prior chain does not contain `InitZomesComplete` (the agent was installed but never made a zome call between genesis and the install on the original node), restore brings back only the genesis prefix. The next zome call after activation runs init normally and appends a fresh `InitZomesComplete` on top of the restored tip. This is correct because the chain on the DHT is well-defined and is simply being extended.

### Partial-arc and zero-arc considerations

Step 1's `get_agent_activity_multi` call routes via the cascade and p2p layers, which already handle the local-vs-remote distinction. It works without modification on a zero-arc node, because the cascade routes to remote authorities for anything not held locally.

For full-arc nodes, gossip will eventually deliver the agent's own ops over the normal sync path as well. Restore deliberately does **not** wait for that — it actively fetches via `get_agent_activity` — because waiting for gossip introduces uncontrolled latency and lacks an explicit termination criterion.

## Required p2p API extension

The current `HolochainP2pActor::get_agent_activity` (`crates/holochain_p2p/src/spawn/actor.rs`) fans out to a weighted set of peers but collapses to the first non-empty response via `select_ok_non_empty`. Restore needs the raw per-peer responses so that the quorum logic above is possible.

Add a new p2p call `get_agent_activity_multi(target_peer_count, min_responses, timeout) -> Vec<AgentActivityResponse>` that returns all responses received within the timeout, up to the target count, paired with the peer that produced each. The existing `get_agent_activity` is left unchanged so application-level callers continue to get the cheap first-non-empty behaviour.

The authority-side handler (`crates/holochain_cascade/src/authority.rs`) needs no changes; it already returns whatever the authority holds.

## What is explicitly not restored

| Data | Storage | Why not restored |
|------|---------|------------------|
| Private entries | `PrivateEntry` table | Never distributed on DHT; only the authoring node ever held them. |
| Cap claims | `CapClaim` table | Local-only index of grants received from other agents; not chain data. |
| Cap grants (entry body) | Action restored, entry body not | `CapGrant` entries are private; the action restores but the entry body cannot be retrieved. The agent loses the ability to exercise prior grants. |
| Received validation receipts | Validation receipt store | Receipts other peers issued for this agent's authored ops; not part of the chain itself. These are rebuilt over time after restore by republishing — see Step 2. |
| Pending counter-signing sessions | `ChainLock` | Transient. Any session in flight on the original node is lost. |

Applications that rely on the lost categories must accept degraded post-restore behaviour, or the layer that triggers restore must arrange an out-of-band channel for them. This design does not attempt to address either.

## Failure modes and operator behaviour

| Failure | Behaviour |
|---------|-----------|
| `agent_key == None` with `restore_from_dht == true` | Reject the install at admit time. |
| `agent_key` not present in local Lair | Reject the install at admit time. |
| Fewer than `restore_chain_quorum` peers respond | Internal retry (see [Retry behaviour](#retry-behaviour)). App stays in `AwaitingRestore`. |
| Responding peers disagree on `ChainHead` | Internal retry. App stays in `AwaitingRestore`. |
| Incomplete chain (gap in `0..=H`) | Internal retry. Re-request missing sequences. App stays in `AwaitingRestore`. |
| Restored record fails signature check against `A` | Record discarded. Restore continues with responses from other peers. |
| Restored record's hash does not match the Step 1 candidate for its sequence | Record discarded. Restore continues with responses from other peers. |
| Warrant returned by a peer for `A` | Submitted to local validation. App stays in `AwaitingRestore` until validation completes; outcome below. |
| Validated `ChainIntegrityWarrant::ChainFork` for `A` | Permanent failure. App transitions to `AppStatus::Unrecoverable(cell_id, ChainFork(...))`; `SystemSignal::RestoreFailed` emitted; remaining cells not attempted. |
| Validated `ChainIntegrityWarrant` of another variant (e.g. `InvalidChainOp`) for `A` | Permanent failure. App transitions to `AppStatus::Unrecoverable(cell_id, ChainIntegrityWarrant(...))`; `SystemSignal::RestoreFailed` emitted; remaining cells not attempted. |
| Warrants all rejected by local validation | Treated as if no warrants were reported. Workflow proceeds to Step 2. |
| Crash mid-restore | Workflow re-enters Step 1 on next conductor start (see [Crash recovery](#crash-recovery)); previously written authored-state rows are reused. |

