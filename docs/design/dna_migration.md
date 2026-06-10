# DNA Migration Design: Chain Switch

## Status

**Draft / proposed.** This document describes the **chain switch** DNA migration
path: an agent closes their source chain under one DNA and opens a new one under
a different DNA, carrying a peer-signed summary of their old state forward.

Chain switch is a path we intend to keep available permanently. Even if we later
design a migration path that behaves differently, chain switch stays, because it
permits migrations **across incompatible conductor versions** — the old and new
DNAs need not run on the same conductor build, since the carried state moves as
opaque, agent-held bytes rather than through a live cross-cell or cross-network
link. That property is valuable and not easily replaced.

Parts of the mechanism already exist in the codebase (the `CloseChain` /
`OpenChain` actions and their HDK functions, and a driving test). The remaining
pieces — a way to carry agent-controlled, signed content from an old DHT into a
new one, and a way for the new DNA to validate that content — are not yet
designed or built. This document covers the whole flow end-to-end as though it
were being designed from scratch, so that the existing pieces and the proposed
additions are described together.

## Motivation

An application sometimes needs to evolve its DNA in a way that changes the DNA
hash: new entry types, changed validation rules, a different integrity zome. A
changed DNA hash means a different network and a different DHT. Agents on the
old DNA cannot simply "keep going" — they must move to the new DNA, and they
usually want to bring a summary of their old state with them.

Two properties matter:

1. **Agent control.** The migrating agent should decide what to carry forward
   and when to do it. Migration must not depend on a third party reaching into
   the new network on the agent's behalf.
2. **Offline friendliness.** Once an agent holds the data they want to carry
   forward, installing and seeding the new DNA must not require the old network
   to be reachable. (See the "Offline friendly" principle in `CLAUDE.md`.)

The current code satisfies neither property for the cross-DHT case, because the
only working path relies on the old and new cells living in the same conductor
so the new cell can make a live zome call back into the old cell.

## Background: open and closed chains

A source chain can be _closed_ to mark the end of authorship under one
DNA/agent, and a new chain can be _opened_ to declare where it migrated from.
These are represented by two system actions.

### `CloseChain`

Defined in `crates/holochain_integrity_types/src/action.rs`:

```rust
pub struct CloseChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub new_target: Option<MigrationTarget>,
}
```

`CloseChain` is committed as the **last** action on the old chain. Its
`new_target` optionally declares the forward migration path. A `CloseChain` with
`new_target == None` simply retires a chain with no forward reference.

System validation enforces that nothing may follow a `CloseChain`. In
`crates/holochain/src/core/workflow/sys_validation_workflow.rs`,
`register_agent_activity` rejects any action whose previous action is a
`CloseChain` with `PrevActionErrorKind::ActionAfterChainClose`.

### `OpenChain`

```rust
pub struct OpenChain {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,

    pub prev_target: MigrationTarget,
    /// The hash of the `CloseChain` action on the old chain, to establish chain
    /// continuity and disallow backlinks to multiple forks on the old chain.
    pub close_hash: ActionHash,
}
```

`OpenChain` is committed (conventionally during `init`) on the new chain. It
declares `prev_target` — where the chain came from — and `close_hash`, the hash
of the matching `CloseChain` action. The `close_hash` binds the new chain to a
single closing action on the old chain, so an old chain cannot fork into
multiple migrated successors.

### `MigrationTarget`

```rust
pub enum MigrationTarget {
    /// A DNA migration: the new or previous DNA hash.
    Dna(DnaHash),
    /// An Agent migration: the new or previous Agent key.
    Agent(AgentPubKey),
}
```

Of the two components of a `CellId` (DNA hash and agent key), a migration holds
one fixed and changes the other. `MigrationTarget` names the component that
changed. This document is concerned with **DNA migration** (`Dna`), where the
agent key is stable and the DNA hash changes.

> Note on signing: for an _agent_ migration (`CloseChain` with
> `MigrationTarget::Agent`), the `CloseChain` action is signed by the new agent
> key rather than the author key (see `Action::signer` in `action.rs`). DNA
> migration does not change the signer.

### HDK surface

`crates/hdk/src/migrate.rs` exposes:

```rust
pub fn close_chain(new_target: Option<MigrationTarget>) -> ExternResult<ActionHash>;
pub fn open_chain(prev_target: MigrationTarget, close_hash: ActionHash) -> ExternResult<ActionHash>;
```

These are thin wrappers over the `close_chain` / `open_chain` host functions in
`crates/holochain/src/core/ribosome/host_fn/`, which append the corresponding
action to the source chain with `ChainTopOrdering::Strict`.

## What exists today, and where it falls short

A worked example lives in the test WASMs and a driving test:

- `crates/test_utils/wasm/wasm_workspace/migrate_initial` — the "old" DNA.
  Its coordinator exposes `close_chain_for_new(dna_hash)`, which calls
  `close_chain(Some(dna_hash.into()))`.
- `crates/test_utils/wasm/wasm_workspace/migrate_new` — the "new" DNA. Its
  `init` reads the previous DNA hash from its **DNA properties**, calls
  `open_chain`, then fetches the old data and re-authors it under the new entry
  type.
- `crates/holochain/tests/tests/migration.rs` —
  `migrate_dna_with_second_app_install` drives the two: create data on the old
  DNA, close the old chain toward the new DNA, install the new app, and assert
  the new cell ends up with both the migrated and the freshly created entries.

This test passes, but only because of two shortcuts that do not generalise:

1. **The new cell reads the old data with a live cross-cell call.**
   `migrate_new::init` does:

   ```rust
   let response = call(
       CallTargetCell::OtherCell(CellId::new(properties.prev_dna_hash, my_agent)),
       "migrate_initial", "get_all_my_types".into(), None, (),
   )?;
   ```

   This works only because both cells are installed in the same conductor under
   the same agent, and the old cell is still present and running. There is no
   path here for carrying data into a _new DHT_ that the old-network peers are
   not part of, and it is not offline friendly — it depends on the old cell
   being live.

2. **The `close_hash` is faked.** `migrate_new/src/coordinator.rs` contains:

   ```rust
   // TODO: must get close_hash from init context, which is currently not possible.
   let close_hash = ActionHash::from_raw_36(vec![0; 36]);
   open_chain(properties.prev_dna_hash.clone().into(), close_hash)?;
   ```

   The new chain cannot currently learn the hash of the `CloseChain` it is
   migrating from, so it commits a zero hash. The continuity guarantee that
   `close_hash` is meant to provide is therefore not real in the current test.

The missing capability is a way for the agent to **carry agent-controlled,
validated content from the old DHT into the new DNA**, available at `init` time,
without the old network being reachable and without handing control to a third
party.

## The problem we are solving

The chain switch migration pattern is:

1. On the old chain, the agent authors a **final summary record** — a
   distillation of whatever state they want to carry forward.
2. That summary is **validated and signed by peers on the old DHT**, who have
   access to the old data and can confirm the summary was constructed correctly.
3. The agent commits `CloseChain` on the old chain, retiring it and pointing at
   the new DNA.
4. The agent keeps the signed summary and **moves it forward** into the new DNA
   they are about to install.

Step 4 is the gap. There is currently **no way to get the signed summary into
the new DHT**.

One option is to have the peers who signed the summary _bridge_ it into the new
DHT. We reject this: it takes control out of the agent's hands and makes
migration depend on those peers participating in the new network.

Instead, the agent should carry the summary themselves and hand it to the new
app at install time. The new app decides how to interpret it and whether to use
it to seed the new chain. The summary's signatures are then checked by the new
DNA's own validation, using a set of trusted signer keys baked into the new
DNA's properties.

## Proposed design

### Overview of the flow

```
OLD DNA (old DHT)                          NEW DNA (new DHT)
-----------------                          -----------------
author summary record
   |
   v
peers on old DHT validate + sign summary
   |
   v
commit CloseChain(new_target = Dna(new_dna_hash))   [last action on old chain]
   |
   |   agent holds: signed summary + close_hash
   v
install new app with `init_properties`  ----------->  conductor persists
(opaque bytes: summary, signatures,                   init_properties in the
 close_hash, ...)                                     CONDUCTOR DB (not the DHT)
                                                          |
                                                          v
                                              init() runs:
                                                - get_init_properties() host fn
                                                  reads the opaque bytes
                                                - app decodes them
                                                - open_chain(prev = Dna(old),
                                                    close_hash = <real hash>)
                                                - app seeds new chain from the
                                                  summary (create_entry, ...)
                                                          |
                                                          v
                                              app validation on the new DHT:
                                                trusts the summary's signatures
                                                iff signed by keys listed in the
                                                NEW DNA's properties
```

The design has four additions on top of the existing open/closed-chain
machinery:

1. A new **`init_properties`** install-app parameter: opaque bytes, per role.
2. **Persistence** of `init_properties` in the **conductor database**, keyed by
   app and role — deliberately _not_ in the DHT.
3. A new **HDK function and host function** to look up the `init_properties`
   from within a zome (primarily during `init`).
4. A **validation convention**: the new DNA lists trusted signer public keys in
   its DNA properties, so the new DNA's app validation can trust signatures on
   carried content that originated on the old network.

Each is described below.

### 1. Producing and signing the summary (application responsibility)

This is application logic and needs no new host capability beyond what exists:

- The old coordinator authors the summary record.
- Peers on the old DHT validate it (ordinary app validation can enforce that the
  summary is well-formed) and return their **signatures** over the summary
  bytes. Collecting signatures from peers is an application-level interaction
  (e.g. a remote signal / `call_remote` request-response). The exact protocol is
  out of scope for this document; the design only requires that the agent ends
  up holding the summary bytes plus a set of `(signer_pubkey, signature)` pairs.
- The agent commits `close_chain(Some(MigrationTarget::Dna(new_dna_hash)))`.
  This yields the **`close_hash`** that the new chain will need.

At this point the agent locally holds everything required to seed the new DNA:
the summary, the signatures, and the `close_hash`. None of the following steps
require the old network.

### 2. `init_properties` install parameter

`install_app` gains a way to pass opaque, app-defined bytes that are made
available to the cell during `init` and afterwards. The natural home, following
the existing per-role model used for membrane proofs and DNA modifiers, is the
`RoleSettings::Provisioned` variant in `crates/holochain_types/src/app.rs`:

```rust
pub enum RoleSettings {
    UseExisting { cell_id: CellId },
    Provisioned {
        membrane_proof: Option<MembraneProof>,
        modifiers: Option<DnaModifiersOpt<YamlProperties>>,
        /// NEW: opaque, app-defined bytes made available to the cell at init
        /// time via the `get_init_properties` host function. Not interpreted by
        /// the conductor and never written to the DHT.
        init_properties: Option<InitProperties>,
    },
}
```

Where `InitProperties` is a newtype around `SerializedBytes` (mirroring how
`MembraneProof` wraps bytes). The bytes are **opaque to the install process**;
the app alone decides how to decode them.

Why a new parameter rather than reusing existing channels:

- **DNA properties (`modifiers.properties`)** are part of the DNA hash. The
  carried summary is per-agent, per-migration content; it must not change the
  DNA hash, so it cannot live in DNA properties. (DNA properties remain the home
  for the _trusted signer keys_ in §4, which _are_ shared across all installs of
  the new DNA.)
- **Membrane proof** is written into the source chain (the `AgentValidationPkg`
  action) and is therefore shared to the DHT. The summary should stay private to
  the conductor unless and until the app chooses to author derived data from it.

So `init_properties` is a third, distinct channel: opaque, optional, per role,
conductor-local.

### 3. Persisting `init_properties` in the conductor database

`init_properties` is stored in the **conductor database**, not in any cell or
DHT database. Rationale: it is conductor-local seed material, it must survive
restarts so that a deferred or re-run `init` can still read it, and keeping it
out of the DHT avoids polluting shared state with per-agent migration payloads.

The conductor DB already stores per-app data. The schema lives under
`crates/holochain_sqlite/src/sql/conductor/schema/` and is accessed through
`crates/holochain_data/src/conductor.rs` (with the store layer in
`crates/holochain_state/src/conductor.rs`). Two implementation shapes are
possible; this document recommends the first:

- **(Recommended) A dedicated table** keyed by `(installed_app_id, role_name)`
  holding the opaque blob, written when the app is installed and deletable when
  the app is uninstalled. This keeps the payload out of the serialized
  `ConductorState` blob and makes lifecycle/cleanup explicit. A new schema
  migration adds the table; reads/writes follow the existing `holochain_data`
  query patterns (e.g. alongside `InstalledApp`).

- **(Alternative) Inside the installed-app record.** Store the blob as part of
  the per-role provisioning data already persisted for the app. Simpler, but
  conflates one-shot seed material with durable app configuration.

Lifecycle:

- **Written** during `install_app_bundle`
  (`crates/holochain/src/conductor/conductor.rs`), where `roles_settings` is
  already destructured (see `get_memproof_map_from_role_settings` etc.). A
  parallel `get_init_properties_map_from_role_settings` extracts the new field.
- **Read** on demand by the new host function (below).
- **Removed** on app uninstall. Whether it is also cleared after a successful
  `init` is an open question (see §"Open questions") — retaining it lets `init`
  be retried idempotently if it fails partway.

### 4. HDK function and host function to read `init_properties`

A new host function exposes the persisted bytes to the running zome. The wiring
follows the existing read-only host functions (`dna_info`, `agent_info`) exactly
— see the end-to-end example in `agent_info` /
`crates/holochain/src/core/ribosome/host_fn/agent_info.rs`.

Proposed surface (HDK, `crates/hdk/src/`):

```rust
/// Look up the opaque init properties supplied to `install_app` for this cell's
/// role, if any. Returns `None` if none were provided.
///
/// Typically called from `init` to obtain seed material carried forward from a
/// previous DNA (e.g. a signed summary and the `close_hash` of the old chain).
/// The bytes are app-defined; the caller is responsible for decoding them.
pub fn get_init_properties() -> ExternResult<Option<InitProperties>>;
```

Data path for the host function:

- The `init` callback runs with `InitHostAccess`
  (`crates/holochain/src/core/ribosome/guest_callback/init.rs`), which already
  carries a `call_zome_handle: CellConductorReadHandle`.
- `CellConductorReadHandleT` (`crates/holochain/src/conductor/api/api_cell.rs`)
  gains a method, e.g. `get_init_properties(&self) -> ConductorResult<Option<InitProperties>>`,
  implemented on `CellConductorApi`. This mirrors the existing
  `find_app_containing_cell` method: it reaches the conductor handle, looks up
  the app/role for the current `cell_id`, and reads the conductor DB.
- The new `host_fn/get_init_properties.rs` reads from
  `call_context.host_context().call_zome_handle()` and returns the bytes.

Permissions: this is a read-only host function. It should be permitted under the
init callback's access (`InitHostAccess` currently grants `HostFnAccess::all()`)
and under ordinary zome-call access. A dedicated `HostFnAccess` flag can be
added if we want to restrict it.

Full wiring checklist (one new host function), per the established pattern:

1. `crates/holochain_zome_types/src/zome_io.rs` — add to the `wasm_io_types!`
   macro.
2. `crates/holochain/src/core/ribosome/host_fn.rs` — add to the
   `host_fn_api_impls!` macro.
3. `crates/holochain/src/core/ribosome/host_fn/get_init_properties.rs` — the
   implementation module.
4. `crates/holochain/src/core/ribosome/real_ribosome.rs` — `use` it and register
   it with `.with_host_function(..., "__hc__get_init_properties_1", ...)`.
5. `crates/hdk/src/hdk.rs` — add the `HdkT` trait method and the `HostHdk` impl.
6. `crates/hdk/src/` — the public HDK wrapper, re-exported from `prelude.rs`.
7. `crates/hdk/src/prelude.rs` — register in the `holochain_externs!` macro.

### 5. Seeding the new chain during `init` (and fixing the `close_hash` TODO)

With `get_init_properties` available, `migrate_new::init` no longer needs the
live cross-cell call or the faked `close_hash`. The new `init` becomes:

```rust
#[hdk_extern]
fn init() -> ExternResult<InitCallbackResult> {
    // Opaque bytes supplied at install time. Absent => fresh install, no migration.
    let Some(props) = get_init_properties()? else {
        return Ok(InitCallbackResult::Pass);
    };
    let seed: MigrationSeed = props.try_into()?;   // app-defined decode

    // The real close_hash now comes from the carried seed, not a zero hash.
    open_chain(MigrationTarget::Dna(seed.prev_dna_hash), seed.close_hash)?;

    // Seed the new chain from the *carried* summary — no call back to the old
    // cell, no dependency on the old network being reachable.
    for item in seed.summary.records {
        create_entry(/* ... derived from item ... */)?;
    }

    Ok(InitCallbackResult::Pass)
}
```

This resolves the `// TODO: must get close_hash from init context` in
`migrate_new/src/coordinator.rs`: the agent placed the real `close_hash`
(returned by `close_chain` on the old chain) into `init_properties`, and the new
chain reads it back here. Because the `OpenChain` action is appended with strict
ordering and `init` commits before `InitZomesComplete`, the migrated records and
the `OpenChain` marker land at the head of the new chain in the expected order.

The summary itself is carried in `init_properties`, so seeding is fully local.
The old cell does not need to be installed or running, and the old network does
not need to be reachable.

### 6. Validating carried content on the new DNA

The new DNA must be able to decide whether to _trust_ the carried summary, since
the summary was constructed and signed on the old network — a network the new
DNA's validators are not part of.

The mechanism: **the new DNA lists the public keys whose signatures it trusts in
its DNA properties** (`modifiers.properties`, readable in validation via
`dna_info()`). These are typically the keys of the old-DHT peers authorised to
sign migration summaries.

During app validation on the new DNA, when an agent authors records derived from
a carried summary, the integrity zome's `validate` callback:

1. Reads the trusted signer key set from DNA properties via `dna_info()`.
2. Verifies the summary's `(signer_pubkey, signature)` pairs over the summary
   bytes using the existing `verify_signature` host function.
3. Accepts the derived records **iff** the signatures are valid and the signers
   are in the trusted set.

This keeps trust explicit and baked into the DNA hash: every agent on the new
network agrees, by virtue of running the same DNA, on which old-network keys are
authoritative for migration summaries. No new core capability is required beyond
`dna_info()` and `verify_signature`, both of which already exist; this is a
documented validation convention plus, optionally, helper types for the
summary/signature envelope.

> Caveat to capture during implementation: the summary records are validated
> against signatures, not against the old DHT (the new validators cannot see the
> old DHT). The trust model is therefore "the listed signers vouched for this
> summary", not "the new network re-derived the summary". This trade-off is
> inherent to chain switch and should be called out in user-facing docs.

## Summary of required changes

Types and HDK:

- `crates/holochain_types/src/app.rs` — add `init_properties: Option<InitProperties>`
  to `RoleSettings::Provisioned`; define `InitProperties` (newtype over
  `SerializedBytes`) and any `RoleSettingsMap` extraction helpers.
- `crates/hdk/src/` (+ `prelude.rs`) — new `get_init_properties` wrapper and
  extern registration.
- `crates/holochain_zome_types/src/zome_io.rs`,
  `crates/holochain/src/core/ribosome/host_fn.rs`,
  `crates/holochain/src/core/ribosome/host_fn/get_init_properties.rs`,
  `crates/holochain/src/core/ribosome/real_ribosome.rs` — host function wiring.

Conductor and persistence:

- `crates/holochain_sqlite/src/sql/conductor/schema/` — schema migration for the
  new init-properties storage (recommended: a dedicated table keyed by app +
  role).
- `crates/holochain_data/src/conductor.rs` (and the store layer in
  `crates/holochain_state/src/conductor.rs`) — read/write/delete operations.
- `crates/holochain/src/conductor/conductor.rs` — extract `init_properties` from
  `roles_settings` during `install_app_bundle`, persist it, and clean up on
  uninstall.
- `crates/holochain/src/conductor/api/api_cell.rs` — `get_init_properties` on
  `CellConductorReadHandleT` / `CellConductorApi`.

Tests and example WASMs:

- `crates/test_utils/wasm/wasm_workspace/migrate_new` — replace the live
  cross-cell `call()` and the zero `close_hash` with `get_init_properties()`-based
  seeding; remove the `TODO`.
- `crates/holochain/tests/tests/migration.rs` — extend
  `migrate_dna_with_second_app_install` (or add a sibling test) to install the
  new app with `init_properties` carrying a signed summary + `close_hash`, and
  assert the new chain is seeded without a live old cell. Add a validation test
  for the trusted-signer-keys path. Prefer inline zomes where wasm execution is
  not itself under test (per `CONTRIBUTING.md`); the existing WASM pair stays for
  the end-to-end case.

Docs:

- Update `crates/holochain/CHANGELOG.md` (new install parameter + host function).
- Cross-reference this document from `docs/design/state_model.md` where chain
  open/close is discussed.

## Non-goals

- Chain switch is not the only migration path we may offer. A future path may
  behave differently; chain switch nonetheless stays available, since it is the
  path that works across incompatible conductor versions. This document does not
  attempt to design any other path.
- Agent-key migration (`MigrationTarget::Agent`) is out of scope here; this
  document addresses DNA migration only.
- Automatic re-validation of the summary against the old DHT from the new
  network is explicitly not attempted (the new validators cannot see the old
  DHT). Trust is delegated to the listed signer keys.

## Open questions

- **Lifecycle of stored init properties.** Should the conductor delete the
  stored bytes after a successful `init`, or retain them so `init` is idempotent
  on retry? Retaining is safer for failure recovery but leaves seed material at
  rest in the conductor DB. Recommendation: retain until uninstall, document it.
- **Size limits.** Carried summaries could be large. Do we cap
  `init_properties` size, and where (admin API, conductor)?
- **Multiple roles.** `init_properties` is per role; confirm the keying and the
  install API ergonomics when an app has several provisioned roles that each
  need seed material.
- **Signer-key revocation.** Trusted signer keys are fixed in the DNA hash. If a
  signer key must be retired, that requires a further DNA migration. Worth
  stating as a known constraint of chain switch.
- **Deferred init.** Interaction with deferred-membrane-proof installs (the app
  enters a deferred state) and whether init properties can likewise be supplied
  after the initial install.
