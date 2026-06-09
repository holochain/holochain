# v2 Data-Model Migration

Status: design / in progress.

This document describes the program of work to make the **v2 action/record
model** (`ActionHeader` + `ActionData`, see `state_model.md`) the single
canonical data model used throughout Holochain, and to delete the legacy
per-variant `Action` model together with all `legacy ↔ v2` conversion code.

It is the umbrella spec for a **single PR delivered in sequential phases**
(commits on one branch), not a series of separate PRs. The phases exist to stage
the work, not to ship independently: only the final state of the branch must
pass the full test suite, so we never have to make tests pass in a transitional,
half-migrated state (the explicit reason for not splitting this into per-phase
PRs). Each phase should still compile; tests may be transiently red between
phases (e.g. while consumers are mid-rewire) and are only required green at the
end of the branch.

Each phase gets its own implementation plan and, where useful, a detailed design
section appended here. The first phase — **Phase 0: finalize the v2 model** — is
specified in full below; later phases are sketched and will be detailed once
Phase 0 builds.

## Background

The persistence refactor (see the in-progress `holochain_data` migration)
introduced a redesigned DHT state model. Its types currently live in
transitional `dht_v2` modules:

- `holochain_integrity_types::dht_v2` — `Action` (`ActionHeader` + tagged
  `ActionData`), the per-variant `*Data` structs, `ActionType`,
  `RecordValidity`/`OpValidity`, `CapAccess`.
- `holochain_zome_types::dht_v2` — re-exports plus `SignedAction` /
  `SignedActionHashed` aliases over the v2 `Action`, and the
  `from_legacy_signed_action` / `to_legacy_signed_action` conversions.
- `holochain_types::dht_v2` — re-exports.

Today the legacy model (`holochain_integrity_types::action::Action`, the
per-variant `Create`/`Update`/`Delete`/`CreateLink`/… structs, and the
`Op`/`FlatOp` validation API built on them) is still the canonical model used
by authoring, validation, the HDK/HDI SDK, and the network wire. The v2 model
is used only for storage in the new unified per-DNA DHT database. The two are
bridged by `legacy ↔ v2` conversions, and the network wire still carries the
legacy encoding, with a hash-preservation hack so that breaking the wire is not
yet required.

The legacy `Action` hash includes the rate-limiting `weight` field; the v2
model deliberately omits it. That difference is the root cause of the
hash-preservation machinery.

### Why migrate fully

- A single model removes the entire `legacy ↔ v2` conversion surface and the
  hash-preservation hack.
- The network wire can carry v2 actions with v2 hashes natively.
- Storage, validation, and the SDK stop disagreeing about what an action *is*.

## End state

- The v2 model is the only action/record model. It lives in the canonical
  modules (`holochain_integrity_types::action`, `::record`, `::op`), with the
  `dht_v2` transitional modules dissolved and the `_v2` suffixes removed.
- The legacy per-variant `Action` enum, the `Create`/`Update`/`Delete`/
  `CreateLink`/`DeleteLink`/`EntryCreationAction` structs, and the `weight`
  machinery are deleted.
- `from_legacy_signed_action` / `to_legacy_signed_action` and every other
  `legacy ↔ v2` conversion are deleted.
- The network wire (K2 op store + application `get` `WireOps`) carries v2
  actions with v2 hashes.

## Program decomposition

Phases, in dependency order, landing as sequential commits on a single branch /
PR. Phases 3 and 4 are merged (the host↔wasm serialization boundary couples
them).

| Phase | Scope | Depends on |
|-------|-------|------------|
| **0. Finalize v2 model** | Ratify `weight` removal; fix canonical module layout; **design and land the v2-native validation API (`Op`/`FlatOp`/`OpType`) and a v2 `Record`**, defined but unwired. | — |
| **1. Cascade reads + wire cutover** | Cascade (authority + local) reads via `DhtStore`/`holochain_data`; break the K2 + application-`get` wire to v2; drop the network-side conversions and hash-preservation. Delivers issue #5730. | 0 |
| **2. Internal DHT consumers → v2** | sys/app validation, integration, publish, validation receipts operate on v2; remove `v2→legacy` reads serving these consumers. | 0 (1 helps) |
| **3+4. Ribosome/host boundary + HDK/HDI SDK** | Host functions and the call-zome workspace operate in v2; settle host↔wasm serialization; migrate the SDK (`hdi`/`hdk`/`hdk_derive`) and **all** test wasms to the v2 validation API. Breaking SDK change. | 0, 2 |
| **5. Authoring pipeline → v2** | Source chain, scratch, signing produce v2 natively; drop the authoring `legacy→v2` conversion in `source_chain.rs`. | 0, 3+4 |
| **6. Delete legacy + conversions** | Remove the legacy `Action`/`weight` types, dissolve `dht_v2`, promote v2 types to canonical module names, delete all conversion functions. | all |

Phases 1 and 2 address the cascade-read goal (the original starting point) and
follow immediately once the model is frozen in Phase 0.

## Cross-cutting decisions

These are fixed for the whole program:

1. **`weight` is dropped.** Rate-limiting by `weight` is inert today
   (`weigh_placeholder()` always yields `EntryRateWeight::default()`, nothing
   computes or enforces a non-zero weight). v2 already excludes it. No weight is
   ever re-added to the v2 hash. The rate-limit scaffolding (`weigh_placeholder`,
   `EntryRateWeight`/`RateWeight`, the `Weighed`/`weightless` trait machinery,
   the `put_weightless`/`put_countersigned` split) is removed as each call site
   is migrated in later phases.

2. **The validation API is redesigned around `ActionData`.** `Op` and `FlatOp`
   expose the v2 `Action`/`Record`/`Entry` directly; validators match
   `ActionData` variants. There are no legacy-shaped typed per-variant action
   structs. This maximizes alignment with the storage model and keeps a single
   source of truth, at the cost of rewriting every validator and the
   `hdk_derive` validate dispatch (absorbed in Phase 3+4).

3. **Module layout: stage in `dht_v2`, promote in Phase 6.** During Phases 0–5
   the legacy types still occupy `action.rs` / `record.rs` / `op.rs`, so the new
   v2 types cannot take the canonical names without colliding. New v2 types land
   under `dht_v2` (Phase 0) and coexist with legacy. Phase 6 deletes legacy,
   dissolves `dht_v2`, and promotes the v2 types into the canonical
   `action`/`record`/`op` modules.

---

## Phase 0 — Finalize the v2 model

### Goal

Freeze the canonical v2 data model and the v2-native validation API so every
later phase builds against a stable, compiled, tested target.

### Deliverable

- This committed design.
- The finalized v2 types landed in code, **defined but not yet wired** into the
  host, SDK, validators, or wire (legacy remains canonical behind the existing
  conversions):
  - a v2 `Record` type (new),
  - the `ActionData`-based `Op` and its variant structs,
  - the `ActionData`-based `FlatOp` / `OpType` / `OpEntry` / `OpRecord` /
    `OpActivity`,
  - unit tests for the above.

Nothing in Phase 0 changes runtime behavior: the new types have no callers yet.

### 1. Weight

No code beyond confirming the v2 `*Data` structs carry no `weight`. The
scaffolding removal is explicitly **out of scope for Phase 0** — it is deleted
in the phases that own the relevant call sites (authoring, host fns, sys
validation). This section records the decision so those phases can act without
re-litigating it.

### 2. v2 `Record`

The legacy `Record` is:

```rust
// holochain_integrity_types::record (legacy)
pub struct Record {
    pub signed_action: SignedActionHashed,        // legacy SignedActionHashed
    pub entry: RecordEntry<Entry>,
}
```

`SignedActionHashed` and the generic `SignedHashed` / `Signed` wrappers are
shared between legacy and v2 (only the inner `Action` differs), and
`RecordEntry` is model-agnostic. The v2 `Record` is the same structure over the
v2 `SignedActionHashed` (`= SignedHashed<dht_v2::Action>`):

```rust
// holochain_integrity_types::dht_v2 (Phase 0)
pub struct Record {
    pub signed_action: SignedActionHashed,        // v2 SignedActionHashed
    pub entry: RecordEntry<Entry>,
}
```

`Entry` and `RecordEntry` are unchanged by this program.

### 3. v2 `Op`

The legacy `Op` (in `holochain_integrity_types::op`) wraps legacy typed
per-variant action structs, e.g. `StoreEntry { action: SignedHashed<EntryCreationAction>, entry }`,
`RegisterUpdate { update: SignedHashed<Update>, .. }`,
`RegisterCreateLink { create_link: SignedHashed<CreateLink> }`. Those typed
structs carry `weight` and are removed under decision (2).

The v2 `Op` carries the v2 `Record` / v2 `SignedActionHashed` directly and
relies on `ActionData` to discriminate variants instead of distinct typed
structs. The variant set is preserved (it mirrors the authority/op semantics,
not the action shape):

- `StoreRecord` — carries the v2 `Record`.
- `StoreEntry` — carries the v2 `SignedActionHashed` (whose `ActionData` is
  `Create` or `Update`) plus the `Entry`.
- `RegisterUpdate` — carries the v2 `SignedActionHashed` (`ActionData::Update`)
  plus the optional new `Entry`.
- `RegisterDelete` — carries the v2 `SignedActionHashed` (`ActionData::Delete`).
- `RegisterAgentActivity` — carries the v2 `SignedActionHashed` plus the
  optional cached `Entry`.
- `RegisterCreateLink` — carries the v2 `SignedActionHashed`
  (`ActionData::CreateLink`).
- `RegisterDeleteLink` — carries the v2 `SignedActionHashed`
  (`ActionData::DeleteLink`).

Where a variant is only valid for a subset of `ActionData` (e.g. `StoreEntry`
requires `Create`/`Update`), construction validates the `ActionData` tag and
returns a typed error rather than panicking. The exact constructor/accessor
surface is an implementation detail for the Phase-0 plan; the requirement is
that a consumer can obtain the `ActionHeader` and the relevant `*Data` for a
variant without a parallel typed-action representation.

### 4. v2 `FlatOp` / `OpType` / `OpEntry` / `OpRecord` / `OpActivity`

`FlatOp<ET, LT>` (in `hdi`) is the ergonomic, app-type-parameterized layer
produced by `op.flattened()?`. It deserializes app entries into the app's
`EntryTypes`/`LinkTypes` and exposes sub-enums `OpRecord<ET, LT>`,
`OpEntry<ET>`, `OpActivity<Unit, LT>`, and inline link-op structs.

The v2 `FlatOp` keeps this parameterization and the `flattened()` entry point,
but its variants and helpers are expressed over v2 `ActionData`:

- Variant payloads that today embed a legacy typed action (e.g. the
  `action: CreateLink` field on `RegisterCreateLink`, the `OpUpdate`/`OpDelete`
  sub-enums) instead expose the v2 `ActionHeader` plus the relevant `*Data`
  (`CreateLinkData`, `UpdateData`, `DeleteData`, …).
- App-entry deserialization (the `ET` projection) and link-type resolution
  (`LT`) are unchanged in intent; only the action representation changes.

The Phase-0 deliverable defines these v2 types and a v2 `flattened()` over the
v2 `Op`, with unit tests. They are **not** wired into the `validate` host
callback or `hdk_derive` in Phase 0; that rewiring and the migration of all
test-wasm validators happen in Phase 3+4.

### Explicitly out of scope for Phase 0

- No removal of legacy types or conversions (Phase 6).
- No changes to the host `validate` dispatch, `hdk_derive`, or any test-wasm
  validator (Phase 3+4).
- No wire-format change (Phase 1).
- No authoring/source-chain changes (Phase 5).
- No removal of the `weight` scaffolding (later phases, per call site).

### Testing

Unit tests co-located with the new types:

- Serialization round-trips for v2 `Record`, `Op`, and `FlatOp`.
- Hashing invariance for the v2 `Record`/`SignedActionHashed` (the stored hash
  matches the content hash; no weight participates).
- `Op` construction rejects mismatched `ActionData` tags (e.g. `StoreEntry`
  built from a `Delete`).
- `flattened()` over representative `ActionData` variants yields the expected
  `FlatOp`/`OpEntry`/`OpRecord`/`OpActivity` shapes, including app-entry and
  link-type projection.

### Risks / open questions

- **`FlatOp` ergonomics.** Matching on `ActionData` is more verbose than the
  current typed-struct matching. Phase 0 should validate the ergonomics on a
  couple of representative validators (as tests) before the SDK phase commits to
  the shape.
- **`hdk_derive` impact.** The macro layer's assumptions about typed per-variant
  actions are not addressed in Phase 0; Phase 3+4 must budget for a non-trivial
  macro rewrite. Phase 0 should note any macro-facing constraints discovered
  while shaping the v2 `FlatOp`.

## Phase 1 — Cascade reads + wire cutover

### Goal

Serve all cascade reads — both the local-first requester path and the
network-serving authority path — from the unified `holochain_data` DHT database
via `holochain_state`, and break the network wire to carry v2 actions with v2
hashes. Delivers issue #5730 and removes the network-side `legacy ↔ v2`
conversions and the legacy-hash-preservation machinery.

### Decomposition

Phase 1 lands as ordered sub-slices on the one branch, each with its own
implementation plan:

| Sub-slice | Scope | Status |
|-----------|-------|--------|
| **1a — `DhtStore` read API** | The cascade's compound read methods on `holochain_state::DhtStore` (+ `holochain_data` primitives), tested in isolation: `retrieve_*`, `get_live_*`, `get_*_details`, `get_links`/`get_link_details`, `get_agent_activity`, `must_get_agent_activity`. | **done** |
| **1b — Data-serving reshape** (authority + `holochain_p2p` + requester) | Reshape the data-serving wire to **records** (actions/entries) + record-level validation status + **warrants**; serve it from the 1a reads on the authority; on the requester, consume it, check the `Rejected ⇒ warrant` invariant, and cache. Delete the legacy authority `Query` structs. | **done** (the legacy authority `Query` structs + cascade integration-test layer are removed; `CascadeTxnWrapper`/`DbScratch` are retained for the requester's local read path — see the deviations note below) |
| **1c — Op + action hash cutover** | Drop hash-preservation: action **and** op hashes become content-derived v2 (no `weight`); op-construction sites build the v2 `dht_v2` op types; the K2 gossip wire carries v2 ops; the network-side `legacy ↔ v2` conversions + the hash-preservation hack are deleted. Two additive foundation slices (1c-i/ii) then one coordinated identity-flip cutover (1c-iii). | designed (see "1c — op + action hash cutover"); 1c-i next |

**Why 1b merges the old "authority" and "requester" slices.** The data-serving
response type is shared by three crates: the authority *produces* it,
`holochain_p2p` *routes/serializes* it, the requester *consumes* it. Reshaping
that type is one atomic change — a phase boundary cannot sit between "authority
produces the new shape" and "requester consumes it" and still compile, and the
only way to split them would be a transitional shim (the kind of scaffolding this
program avoids). So 1b is a single slice delivered as phased commits that compile
at the end of the slice. The old "break the application-`get` wire in 1d" work is
absorbed into 1b; the old gossip wire-break becomes the new final phase **1c**.

### Read-stack architecture

Three layers, with resolution work placed by cost:

- **`holochain_data`** — SQL read operations against the unified DB, resolved in
  SQL *where the SQL stays clean*: live entry (entry with no live delete), live
  links (create-links minus tombstones), "deletes/updates for X",
  agent-activity row scans. Where answering a cascade request would require an
  overly complex multi-join, `holochain_data` returns the component pieces and
  the next layer assembles. These queries are tested in isolation at the DB
  level.
- **`holochain_state`** — the compound "store" layer. `DhtStore` exposes typed
  read methods that call `holochain_data`, **assemble multi-piece results in
  memory** (e.g. record/entry details = action + entry + deletes + updates), and
  — for the requester path only — **overlay the scratch in memory**. The scratch
  type already lives in `holochain_state`, so the in-memory merge is at home
  here.
- **`holochain_cascade`** — thin: call the `holochain_state` read, fall back to
  the network on a miss, cache the network result into the store
  (`DhtStore::cache_chain_ops` / `cache_warrants`, already present). The legacy
  `CascadeTxnWrapper` / `DbScratch` / cross-DB merge machinery is deleted.

### Invariant: scratch is requester-only

The scratch holds *this* conductor's in-flight, uncommitted authored data during
a zome call or validation. It is overlaid **only** on the requester read path.
The authority handlers, which serve requests arriving over the network from other
agents, **must never** read the scratch — peers must not observe uncommitted
local writes. Concretely (within 1b): the authority handlers use the store-only
`DhtStore` reads; the scratch-overlay read variant is used exclusively by the
requester path.

### Return-type boundary (legacy until later phases)

The 1a read methods return the types today's consumers expect — **legacy**
`Record` / `Details` / `Vec<Link>` / `AgentActivityResponse` — converting
`v2→legacy` *inside* the read boundary, with the **legacy hash preserved** (the
hash-preservation hack keeps stored v2 actions carrying the legacy hash). 1b
changes the data-serving wire *shape* (records + record-level validation status +
warrants, see below) but the served actions stay legacy-typed and legacy-hashed.
**1c** then drops hash-preservation and flips the store to native v2 hashes; the
gossip op-wire carries v2, and — because the served actions now carry v2 hashes —
the data-serving wire follows. The network-side `legacy ↔ v2` conversions are
deleted at that point. The remaining `v2→legacy` at the zome-call return boundary
is removed in the merged Phase 3+4.

### 1a — the `DhtStore` read API (specified first)

Add to `DhtStore` (with `holochain_data` primitives as needed), each tested in
isolation:

- `get_live_entry` / `get_live_record` — CRUD-resolved (not deleted).
- `get_entry_details` / `get_record_details` — action + entry + deletes +
  updates, assembled in `holochain_state`.
- `get_links` (live) / `get_link_details` (with tombstones).
- `get_agent_activity` / `must_get_agent_activity`.
- `retrieve_action` / `retrieve_entry` / `retrieve_record` — raw, no CRUD
  resolution.
- a scratch-overlay read variant (or scratch parameter) used only by the
  requester path.

The exact method signatures mirror what `holochain_cascade`'s `get_*`/`retrieve_*`
and `authority::handle_*` consume today (legacy return types) and are pinned in
the 1a implementation plan.

### 1a agent activity — `get_agent_activity` + `must_get_agent_activity`

Agent activity is the highest-risk 1a read, so its design is pinned here. It
splits into two **store-only** DhtStore reads (no scratch — the scratch overlay,
network fallback and cross-source merge are deferred to 1c per the requester-only
invariant):

- **1a-viii — `get_agent_activity`** (the authority "summary" read).
- **1a-ix — `must_get_agent_activity`** (the validation "completeness" read,
  dht-only core).

Both are designed here; **1a-viii is executed first**, then 1a-ix.

**What the reads compute.** Today the work is split across
`holochain_cascade::authority::get_agent_activity_query` (`hashes.rs`,
`records.rs`, the shared `State`/`fold`/`render`, `compute_chain_status`,
`compute_highest_observed`) and the `must_get_agent_activity` module. The only
DB work is a scan of the author's `RegisterAgentActivity` ops ordered by seq;
**everything else is pure in-memory**: classify each op into
valid (`Accepted`) / rejected (`Rejected`); detect a fork (two actions at the
same seq); compute the `ChainStatus` (`Valid`/`Invalid`/`Forked`/`Empty`) and
`HighestObserved`; apply the `ChainQueryFilter`; emit
`ChainItems::Full`/`Hashes`. Warrants are fetched **separately** and attached
(the `Warrant` arm of the legacy row-mapper is dead code —
`WHERE DhtOp.type = RegisterAgentActivity` excludes them). The two legacy
`Query` structs differ only in whether they fetch the entry.

**Decision — integrated-only (drop pending).** The v2 schema splits ops into
`ChainOp` (integrated + validated; `validation_status`/`when_integrated` both
`NOT NULL`) and `LimboChainOp` (pending validation). The v2 reads scan **`ChainOp`
only**, so they report only validated activity. This drops the legacy behavior
where pending (not-yet-integrated) ops raised `highest_observed` — an authority
now advertises only validated state. (Behavior change; acceptable mid-migration.
Any end-state test asserting that pending raises `highest_observed` is updated in
the final-green phase.) Consequence: there is no `pending` list; classification
is purely `Accepted`→valid / `Rejected`→rejected, and `highest_observed` derives
from the valid + rejected lists.

**SQL-vs-Rust boundary.** `holochain_data` does only the scans (trivial SQL: one
author, one op type, `ORDER BY seq`). The pure assembly functions
(`fold`/`render`/`compute_chain_status`/`compute_highest_observed`/
`ChainItemsSource`, and the `must_get` pure helpers `exclude_forked_activity`/
`apply_timestamp_filter`/`check_agent_activity_completeness`/
`collect_canonical_chain_hashes`) **move from `holochain_cascade` into
`holochain_state`** — they are pure and depend only on types.

**1a-viii — `get_agent_activity`:**
- `holochain_data`: one **rich activity-scan** primitive returning, per integrated
  `RegisterAgentActivity` op, the v2 action + its `ChainOp.validation_status`
  (`Accepted`/`Rejected`), with a `LEFT JOIN Entry` parameterized by an
  `include_entries` flag (one query, no N+1, mirroring legacy `records.rs`). It
  reads `ChainOp ⋈ Action` filtered to `op_type = RegisterAgentActivity` and
  `Action.author = :author`, ordered by `Action.seq`. Warrants reuse the existing
  `get_warrants_by_warrantee` primitive (integrated warrants = legacy
  `get_warrants_for_agent(.., check_validity = true)`).
- `holochain_state`: a single `DhtStore::get_agent_activity(author, filter,
  options)` taking a `holochain_state`-local options struct (4 bools — because
  `GetActivityOptions` lives in `holochain_p2p`, which `holochain_state` does not
  depend on; cascade maps it in 1b/1c). It branches `Full`/`Hashes` on
  `include_full_records` (replacing the two legacy `Query` structs), classifies
  rows into valid/rejected, runs the pure assembly, attaches v2→legacy warrants
  (reconstructed from `WarrantRow`) when `include_warrants`, and returns legacy
  `AgentActivityResponse` (legacy `Record`/`ActionHash`; `ActionHash` is the
  preserved legacy hash).
- No authority validity-guard is needed (unlike links): the scan reads `ChainOp`
  (integrated + validated) joined to `Action`, not a cache-populated index.

**1a-ix — `must_get_agent_activity` (dht-only core):**
- `holochain_data`: a `get_filtered_agent_activity` primitive porting the legacy
  `MUST_GET_AGENT_ACTIVITY` query (author + `chain_top` seq bound + optional
  `until` seq lower-bound), returning v2 actions.
- `holochain_state`: `DhtStore::must_get_agent_activity(author, ChainFilter)`
  resolving the `chain_top` to its seq, running the scan, then the pure
  `exclude_forked_activity` → `apply_timestamp_filter` →
  `check_agent_activity_completeness` pipeline, returning
  `MustGetAgentActivityResponse` (the dht-only result; entries stay `None` as
  today). The scratch overlay, network fallback and `merge_*` orchestration
  remain in `holochain_cascade` for the requester half of 1b.

### 1b — data-serving model

1b reshapes the network data-serving path (the `get` / `get_links` /
`get_agent_activity` / `must_get_agent_activity` request handlers, distinct from
gossip) around a single principle: **data serving is about records, not ops.**

**What is served.** Responses carry **records** — `SignedActionHashed` plus the
`Entry` where applicable — *not* the op-shaped wire forms (`WireDelete`,
`WireUpdateRelationship`, `WireNewEntryAction`, `WireCreateLink`,
`WireDeleteLink`), which are dropped. Each served record carries its
**record-level validation status** (`Valid`/`Rejected`) — not a per-op status —
because a caching requester benefits from learning validity immediately. The
response also carries **warrants**.

**Invalid ⇒ warrant (the pairing invariant).** A `Rejected` record is always
served together with a warrant that proves the rejection. The authority holds
that warrant (it warranted the author) and must include it. This is what lets the
receiver trust a `Rejected` verdict without re-deriving it.

**Receiver behaviour (the requester half of 1b).** On a response the requester:
1. **Checks the invariant up front** — any `Rejected` record lacking a paired
   warrant ⇒ the whole response is rejected as malformed/malicious. A lying peer
   cannot force pointless validation work.
2. For **`Rejected`+warranted** records → place into validation limbo paired with
   the warrant (no re-validation).
3. For **`Valid`** records → **expand to ops** with `produce_ops_from_record`,
   keep the op(s) matching the request type and — when *storing* rather than
   caching — the further ops whose `dht_basis` falls in the node's storage arc,
   then run those ops through the normal sys/app-validation → integration
   pipeline. This op-expansion + local re-validation *is* the trust boundary: the
   authority's verdict is a hint, not authority.

**Gossip is unaffected.** Gossip stays op-based (its v2 cutover is 1c). The two
wires are deliberately separate: gossip moves ops between stores; data serving
answers a specific request for immediate use.

**Authority reads.** The authority handlers serve from the 1a `DhtStore` reads
(record/entry/link-shaped, integrated-only) plus a warrant fetch for any rejected
record; the wire types (`WireRecordOps`/`WireEntryOps`/`WireLinkOps`) are
repurposed to the record-shaped form above. No op-shaped, per-op-status reads are
introduced.

### The reshape itself (the 1b-vi cutover) — done

The authority-serving `DhtStore` reads (`get_authority_link_creates`/
`get_authority_delete_links`, `get_authority_store_record`/`…deletes_for_record`/
`…updates_for_record`, `get_authority_entry_creates`/`…deletes_for_entry`/
`…updates_for_entry`), all `locally_validated = 1`-guarded and returning `(legacy
SignedActionHashed, ValidationStatus)`, were done in the preceding sub-slices. The
wire cutover then landed as **one atomic commit** across `holochain_types`,
`holochain_cascade`, and `holochain_p2p`:

- **Wire structs** (`holochain_types`): `WireRecordOps`/`WireEntryOps`/`WireLinkOps`
  now carry `Judged<SignedAction>` (the `Judged` wrapper carries the record-level
  validation status) plus a `warrants: Vec<SignedWarrant>` field; the op-shaped
  sub-types are no longer used by them. Each `render` rebuilds the request-relevant
  op per served action (`RenderedOp::new` with the role's `ChainOpType`) — the
  served records are already full actions, so no op-shaped reconstruction and **no
  `produce_ops_from_record`** are needed. (The full arc-driven
  `produce_ops_from_record` expansion is the future "GET stores by arc" path; today
  GET only caches the request-relevant ops and gossip fills the authoritative
  store.)
- **Authority handlers** (`holochain_cascade::authority`): rewritten onto
  `DhtStoreRead` — `handle_get_record`/`handle_get_entry`/`handle_get_links`
  assemble the reshaped response from the `get_authority_*` reads + `retrieve_entry`
  + a warrant (via the new `DhtStoreRead::get_warrants_by_warrantee`) for any
  `Rejected` record; `handle_get_agent_activity`/`handle_must_get_agent_activity`/
  `handle_get_links_query` call the 1a reads. The production caller (`Cell`) passes
  `space.dht_store.as_read()`.
- **`holochain_p2p` routing**: the empty-response retry check (`spawn/actor.rs`)
  destructures the new fields (a response carrying only warrants is not "empty").
- **Requester** (`holochain_cascade`): extracts the response warrants, checks the
  `Rejected ⇒ warrant` invariant up front (dropping a response that serves a
  rejected record without proof), then caches `Valid` ops + the warrants
  (`locally_validated = 0`).
- **Deletions**: the legacy authority `Query` structs (`GetEntryOpsQuery`/
  `GetRecordOpsQuery`/`GetLinksOpsQuery`/`GetAgentActivity{Hashes,Records}Query`),
  the dead shared agent-activity query helpers, and the entire legacy cascade
  integration test layer (`PassThroughNetwork` + the op-shaped test-data builders +
  the `tests/` suite) — the harness was legacy-`DbKindDht`-based and could not be
  bridged to a `holochain_data` `DhtStore`. Focused new-path unit tests cover the
  `Rejected ⇒ warrant` invariant.

**Deviations from the original sketch / follow-ups:**

- `CascadeTxnWrapper`/`DbScratch`/the cross-DB merge are **retained**: the
  requester's local *read* path (`dht_get`/`dht_get_links` reading post-cache) still
  uses them. Cutting that read path over to `DhtStore` reads is a separate, larger
  effort (a later sub-phase), not part of the data-serving wire reshape.
- The orphaned op-shaped sub-types (`WireDelete`/`WireUpdateRelationship`/
  `WireNewEntryAction`/`WireCreateLink`/`WireDeleteLink` + `WireActionStatus`) are
  left in `holochain_types::action` as dead (public) legacy, to remove in a cleanup
  sweep — they are entangled with still-shared helpers.
- Rebuilding cascade-level integration coverage on a `DhtStore`-backed harness is a
  follow-up; agent-activity serving is covered by the `holochain_state` `DhtStore`
  read tests (1a-ix).

**Invariant assumption:** a `Rejected` record served on the get path carries a
warrant; if app-validation rejections do not always produce a warrant, that is a
validation-layer gap to address separately. Routing rejected+warranted records into
validation limbo (rather than only caching the warrant) is also deferred.

### 1c — op + action hash cutover (full v2 op + gossip wire)

1c drops hash-preservation entirely: action hashes (no `weight`) **and** op hashes
become **content-derived over the v2 form**, the op-construction call sites build the
v2 `holochain_types::dht_v2` op types (`ChainOp`/`HashedChainOp`/`DhtOp`), the gossip
wire encodes v2 ops, and the network-path `legacy ↔ v2` conversions + the
"hash-preservation hack" are deleted. The scope was deliberately expanded from a
narrow gossip-wire byte-swap to the full op cutover, because you cannot move the wire
to v2 ops while keeping legacy hashes: the receiver must recompute the op id from the
bytes it receives, and it cannot reproduce a weight-bearing legacy hash from
weightless v2 data.

**Why this is a coordinated identity flip, not additive reads.** Hashes are DB keys
and references (`prev_action`, op basis). A partial flip leaves references that don't
match, so unlike 1a/1b this cannot stay green incrementally: the v2-hash machinery is
built additively first, then **one coordinated cutover** switches identities across
authoring + state + incoming + publish + gossip together (compile-only intermediate,
green at the end — the 1b-vi discipline). Existing in-dev data is wiped (hard break,
no migration).

**What already exists** (Phase 0 staging): the v2 op types
(`holochain_types::dht_v2::{ChainOp, OpEntry, WarrantOp, DhtOp, HashedChainOp}`) are
defined-but-unwired, and the v2 `Action` is `HashableContent` — so content-derived v2
*action* hashes work today; the system merely *preserves* the legacy hash via
`from_legacy_signed_action`'s `with_pre_hashed`. **Missing:** a content-derived v2
*op* hash — `HashedChainOp.op_hash` has no producer yet.

**One transitional boundary stays:** zome-call authoring still constructs *legacy*
actions (its flip is Phase 3+4), so the produce-ops seam converts legacy→v2 and hashes
v2. That conversion is expected cruft that later phases remove.

Decomposition — two additive (independently green) foundation slices, then one
coordinated cutover:

- **1c-i — v2 op-hash foundation** (additive, green). A v2
  `ChainOpUniqueForm`-equivalent (or op-hash fn) over the v2 `ChainOp` variants →
  `DhtOpHash`, content-derived, no `weight`; plus a `HashedChainOp` constructor filling
  `op_hash`/`basis_hash`/`storage_center_loc` from a v2 `SignedActionHashed` +
  `op_type` + entry. Unit-tested, unwired.
- **1c-ii — v2 produce-ops** (additive, green). A v2 analog of
  `produce_ops_from_record` → `Vec<HashedChainOp>` (legacy→v2 conversion at the seam,
  hashed v2). Additive, unwired, tested.
- **1c-iii — the coordinated cutover** (one PR slice, several commits, green only at the
  end). Stop preserving legacy hashes; compute + store content-derived v2 action+op
  hashes everywhere; move the network onto v2:
  - *Write side:* authoring (genesis / init-zomes / call-zome produce-ops) + state layer
    (`record_incoming_ops`/`cache`/`integrate`/action-indexes/`mutations`) produce and
    store v2 hashes; drop preservation in `from_legacy_signed_action`.
  - *Network side:* gossip `op_store` encodes/decodes v2 `DhtOp`, op id from a native v2
    rehash (delete `build_chain_dht_op` + the "never rehash" hack); `incoming_dht_ops`
    decodes v2 → `HashedChainOp`; publish builds v2 ops.
  - *Validation/ribosome + cleanup:* sys/app-validation, warrants, `must_get_*` host fns
    hash via v2; delete the now-dead legacy↔v2 network conversions + preservation paths.

**Caveat to resolve at 1c-iii planning (not now):** exactly how the source-chain /
authoring path mints the `ActionHash` during chain-building is the riskiest corner of
the write-side flip and has not been traced yet; a focused investigation leads into
planning 1c-iii. **Foundation-first:** plan + execute 1c-i, then 1c-ii, then investigate
the source-chain seam and plan 1c-iii.

### Explicitly out of scope for Phase 1

- The non-cascade internal DHT-read consumers' **read logic** (sys/app-validation
  queues, validation receipts, the incoming-ops intra-batch dedup) — **Phase 2**.
  1c touches these workflows only where they **construct or hash ops** (so the new
  hashes/wire are produced consistently); their read-consumer logic keeps reading
  the v2 store as it does now until Phase 2.
- The zome-call/HDK return boundary `v2→legacy` conversion, and the zome-call
  authoring path still constructing *legacy* actions (1c converts legacy→v2 at the
  produce-ops seam rather than changing how zome calls author) — **Phase 3+4**.

### Testing

- `holochain_data`: DB-level isolation tests for each resolved query (live
  entry/record, live links, link details, deletes/updates-for, agent-activity).
- `holochain_state`: `DhtStore` read tests covering multi-piece assembly and the
  scratch overlay (including the invariant that the store-only variant ignores
  any scratch).
- `holochain_cascade`: local-hit (store), miss-then-network, and cache-write
  behavior; authority handlers serve from the store with no scratch.
- 1b: the data-serving wire round-trips records + validation status + warrants
  (covered by the `holochain_p2p` get round-trip tests); the receiver's invariant
  check (rejected ⇒ warrant) behaves (covered by focused `holochain_cascade` unit
  tests). Routing rejected+warranted records into limbo is deferred (see the
  follow-ups note above).
- 1c: wire round-trip for the v2-encoded K2 gossip ops; v2 hashes end-to-end
  without the legacy-hash hack.

### Risks / open questions

- **Agent activity** is the most complex read (sequence scans, warrants,
  completeness, fork handling). 1a should treat `get_agent_activity` /
  `must_get_agent_activity` as the highest-risk method and may itself warrant a
  dedicated plan within 1a.
- **`holochain_data` vs `holochain_state` line.** The "clean SQL vs in-memory
  assembly" split is a judgement call per query; the 1a plan fixes the line for
  each method and notes any query pushed to in-memory assembly to avoid an
  overly complex join.

## References

- `docs/design/state_model.md`, `docs/design/data_model.md`
- GitHub issue #5730 (serve cascade requests from the new DHT database)
- `crates/holochain_integrity_types/src/dht_v2.rs` (current v2 action model)
- `crates/holochain_integrity_types/src/op.rs`, `crates/hdi/src/flat_op.rs`
  (current validation API)
- `crates/holochain_zome_types/src/dht_v2.rs` (current conversions)
