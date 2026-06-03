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

## References

- `docs/design/state_model.md`, `docs/design/data_model.md`
- GitHub issue #5730 (serve cascade requests from the new DHT database)
- `crates/holochain_integrity_types/src/dht_v2.rs` (current v2 action model)
- `crates/holochain_integrity_types/src/op.rs`, `crates/hdi/src/flat_op.rs`
  (current validation API)
- `crates/holochain_zome_types/src/dht_v2.rs` (current conversions)
