# Coordinator Upgrades Design

## Status

**Draft / proposed.** This document describes **coordinator upgrades**: replacing
the coordinator code of an installed app with a new version, at the app level,
without touching the integrity rules, the network, or the source chain.

It modernises an older "Update Coordinators" spec that predated large parts of
the current system (the unified per-DNA database, the v2 model types, and the
[chain continuation](./dna_migration_chain_continuation.md) migration design). That
older draft was also internally inconsistent on several points; this document
resolves those, records the decisions taken, and lists what is deliberately left
open.

Coordinator upgrades are intended to ship **before** chain continuation and do
**not depend on it**. The two must not conflict, because an app release will
often carry *both* a new coordinator set and a new integrity version. This
document therefore designs coordinator upgrades so that `update_app` leaves a
clean **seam** for an integrity-version change to ride through to chain
continuation, rather than forbidding it (see
[Relationship to chain continuation](#relationship-to-chain-continuation)).

## Terminology

- An **integrity zome** defines entry/link types and the `validate` rules that
  govern them. Integrity code is what fixes the `DnaHash` and is validated
  against; it changes rarely and, when it does, that is a *migration* (chain
  continuation or chain switch), not a coordinator upgrade.
- A **coordinator zome** exposes the callable zome functions an app and its UI
  use. Coordinators hold no validation authority and do not affect the
  `DnaHash`; they may be replaced freely.
- A **coordinator set** is the collection of coordinator zomes bound to one role
  of an installed app. A cell runs exactly **one** coordinator set at a time —
  the latest installed.
- A **coordinator upgrade** replaces the coordinator set of an installed app with
  a new one. Integrity code, network, agent key, and source chain are untouched.
- A **DNA migration** changes the integrity rules — via chain continuation (same
  chain, new integrity version) or chain switch (new chain/network). Out of scope
  here except at the seam.

## Motivation

Coordinator code is where an app's behaviour lives and where most iteration
happens: new zome functions, bug fixes, changed call surfaces. None of that
should require a new network, a new chain, or re-validation of existing data —
coordinators do not validate and do not affect the `DnaHash`.

Today the only shipped path is `update_coordinators(dna_hash, coordinator_bundle)`
(`AdminRequest::UpdateCoordinators`). It is a low-level, **per-DNA** operation:
it targets a `DnaHash` directly and knows nothing about apps, roles, or the
capability grants an app relies on. That has three problems:

1. **It is not app-aware.** An app is the unit a user installs, updates, and
   reasons about. Upgrading coordinators one DNA hash at a time does not match
   how apps are released.
2. **It says nothing about capabilities.** Swapping coordinator code can silently
   change which zome functions exist, orphaning cap grants that referenced them,
   with no defined behaviour.
3. **It cannot co-ordinate a combined release.** A real app update frequently
   changes coordinators *and* integrity together; a per-DNA coordinator swap has
   no place to carry the integrity change.

This design adds an app-level `update_app`, defines coordinator/capability
lifecycle across an upgrade, and keeps `update_coordinators` as the low-level
primitive `update_app` is built on.

## Relationship to chain continuation

Coordinator upgrades and chain continuation are complementary and are designed to
compose into a single app release.

| | Coordinator upgrade | Chain continuation |
|---|---|---|
| Changes | Coordinator zomes only | Integrity version (rules) |
| `DnaHash` / `IntegrityHash` | Unchanged | New `IntegrityHash` |
| Network / chain / key | Unchanged | Network unchanged; chain continues via `ContinueChain` |
| Validation of existing data | Unaffected | Old data validated by old rules |
| Ships | First, standalone | Later |

Chain continuation already establishes two facts this design builds on:

- **One coordinator set, reads every version.** A cell runs a single coordinator
  set (the latest); that coordinator must read data authored under every
  integrity version the chain has used, via app-owned self-tagging enums.
  Coordinator upgrades are how that single set is advanced.
- **Coordinators do not accumulate.** Only integrity versions accumulate in a
  lineage; coordinator zomes are replaced wholesale. So a coordinator upgrade is
  a *swap*, never an append.

The seam that keeps them compatible:

- `update_app` accepts an app bundle that may carry a **new integrity version as
  well as** a new coordinator set. Coordinator upgrade handles the coordinator
  swap; the integrity change is **passed through** to the migration path (chain
  continuation) rather than rejected. Until chain continuation lands, `update_app`
  rejects an integrity change with a clear "migration not yet supported" error —
  it never silently drops it, and it never bakes in the assumption that integrity
  *cannot* change (which the older draft did, and which directly contradicts
  continuation).
- The **upgrade hook** introduced here (see [init and the upgrade
  hook](#init-and-the-upgrade-hook)) is the shared place for post-upgrade setup.
  Chain continuation explicitly deferred a dedicated upgrade callback and left
  "seed any content a new integrity version needs" as manual app work; this
  hook becomes that home too.

## Scope and simplifying decisions

The following decisions shrink the design surface. One proposed simplification —
dropping multiple integrity zomes per DNA — was checked against the current
system and real apps and **rejected**; it is recorded here as decision 1 with the
evidence, so the choice is not silently revisited.

1. **Keep multiple integrity zomes per DNA.** A DNA may declare more than one
   integrity zome, as today. This was considered for removal (to collapse "which
   integrity zome owns this type/dependency" to a non-question), but two
   independent checks killed that idea:
   - **The system handles it correctly.** Entry/link types are namespaced
     per-zome — an `AppEntryDef` carries `(zome_index, entry_index)` and
     `ScopedZomeTypes<T>` is `Vec<(ZomeIndex, Vec<T>)>` — and app-validation
     dispatch routes each op to its *defining* integrity zome by `zome_index`
     (`get_zomes_to_invoke` → `get_integrity_zome_from_ribosome`). There is a
     passing test, `test_coordinator_zome_update_multi_integrity`. It is not a
     degenerate or broken feature.
   - **Real apps depend on it heavily.** ~half of the DNA manifests across
     `lightningrodlabs` and `holochain-open-dev` (23 of 46 scanned) declare more
     than one integrity zome — the dominant idiom being a shared
     `profiles_integrity` (plus `notifications`, `attachments`, `custom_views`,
     `file_storage`, `syn`) bundled beside the app's own integrity zome. Removing
     support would break the majority of those apps.

   This also matches chain continuation's own wording — "one immutable **set** of
   integrity zomes" — so keeping the set (not a single zome) is the consistent
   choice. An integrity version is therefore the DNA's *set* of integrity zomes.
2. **Leave `network_seed` and `properties` where they are.** The current
   layering already supports both: the DNA manifest holds them as defaults, and
   the app role's `dna.modifiers` overrides them (with a further override at
   install). This works and does not need churning, so it is kept as-is rather
   than moving modifiers to a single home. This is continuation-safe: a different
   `properties`/seed choice yields a different `NetworkHash` — i.e. a different
   network — so there is no silent divergence of validation rules within one
   network, wherever the value was set.
3. **Coordinators are bound to the app, not the DNA.** The coordinator set is
   declared per role in the app manifest, not inside the DNA bundle. This is what
   makes a coordinator a swappable, app-owned thing and is the precondition for
   app-level `update_app`.
4. **Keep coordinator `dependencies`, but make it implicit for the single-zome
   case.** Because a DNA may hold several integrity zomes (decision 1), a
   coordinator must still be able to declare which integrity zome's types it
   uses. The `dependencies` field is retained (at most one entry, as today). When
   the DNA has exactly one integrity zome the dependency is implicit and the field
   may be omitted — which is the common case and covers the desired ergonomics
   without dropping the feature multi-zome DNAs need.
5. **Move `clone_limit` to the role.** `clone_limit` is a provisioning concern,
   not a DNA property; it moves from the DNA sub-block to the role.
6. **Flatten the coordinator declaration.** With nothing else expected beside the
   zome list, the coordinator declaration is the list itself rather than a
   `zomes:`-nested object.

## Manifest changes

### DNA manifest — coordinators move out

The DNA manifest loses coordinators (moved to the app). It keeps its
`network_seed`/`properties` defaults (decision 2) and its integrity zome **list**
(decision 1): a DNA may still declare several integrity zomes.

Before (today, abbreviated):

```yaml
manifest_version: "0"
name: group
integrity:
  network_seed: 00000000-0000-0000-0000-000000000000
  properties: ~
  zomes:
    - name: group_integrity
      path: ../target/.../group_integrity.wasm
    - name: profiles_integrity
      path: ../target/.../profiles_integrity.wasm
coordinator:
  zomes:
    - name: group
      path: ../target/.../group_coordinator.wasm
      dependencies:
        - name: group_integrity
```

After (proposed):

```yaml
manifest_version: "1"
name: group
integrity:            # modifiers kept as defaults; only coordinators removed
  network_seed: 00000000-0000-0000-0000-000000000000
  properties: ~
  zomes:              # list retained — a DNA may have several integrity zomes
    - name: group_integrity
      path: ../target/.../group_integrity.wasm
      hash: ~         # optional pin (lock/verify); never author-required
    - name: profiles_integrity
      path: ../target/.../profiles_integrity.wasm
# coordinator: block removed — coordinators are declared in the app role
```

### App manifest — role owns the coordinator set and clone limit

The role gains the coordinator set and `clone_limit`; the DNA sub-block keeps
where to find the DNA and its modifier overrides (seed/properties), as today.

Before (today, abbreviated):

```yaml
manifest_version: "0"
roles:
  - name: group
    provisioning:
      strategy: create
      deferred: false
    dna:
      path: ../path/to/group.dna
      modifiers: { network_seed: ~, properties: ~ }
      installed_hash: ~
      clone_limit: 5
    # coordinators live inside the DNA bundle, not here
```

After (proposed):

```yaml
manifest_version: "1"
roles:
  - name: group
    provisioning:
      strategy: create
      deferred: false
    clone_limit: 5                     # moved up to the role
    dna:
      path: ../path/to/group.dna
      installed_hash: ~
      modifiers:                       # overrides the DNA's seed/properties defaults
        network_seed: ~
        properties: ~
    coordinators:                      # flattened list, bound to the app
      - name: group
        path: ../target/.../group_coordinator.wasm
        hash: ~                        # optional pin
        dependencies: [group_integrity]  # which integrity zome's types it uses;
                                         # omit when the DNA has one integrity zome
```

Notes:

- `hash` on a zome stays **optional** — a pin for lock/verify, never something an
  author has to compute by hand (matching the old draft's "locking is a
  nice-to-have, not forced").
- Seed/properties keep their existing two-layer form: defaults in the DNA
  manifest, overridden by the role's `dna.modifiers`, with a further override at
  install. This layering is unchanged by this design.

## The `update_app` API

```
update_app(installed_app_id, app_bundle) -> AppInfo
```

`update_app` takes an installed app and a new app bundle describing the **complete
desired state** of that app, and reconciles the installation to it.

### Update means strict state

The new bundle is the full intended coordinator/role state, not a delta. There is
no partial "add these coordinators" mode. This removes the older draft's
unresolved "strict state vs subset of state" ambiguity and the dangling/orphaned
coordinator problem that came with the subset interpretation.

### Reconciliation algorithm

For each role in the new bundle:

1. **Integrity.** Compute the role's integrity version from the bundle.
   - If it matches the installed integrity version → no migration; proceed to
     coordinators.
   - If it differs → **pass through to the migration path** (chain continuation).
     Until continuation ships, return a `MigrationNotSupported` error naming the
     role. `update_app` never silently ignores an integrity change and never
     assumes integrity is immutable.
2. **Coordinators (strict).** Reconcile the installed coordinator set to the
   bundle's set for this role, by coordinator name:
   - Present in both → **replace** the coordinator WASM under that name.
   - In the bundle only → **install** the new coordinator.
   - Installed only, absent from the bundle → **remove** it. (This closes the
     older draft's `WHAT TO DO?????` orphan hole: strict state removes orphans.)
3. **Capabilities.** Coordinator changes are followed by the capability
   reconciliation described in [Coordinators and
   capabilities](#coordinators-and-capabilities).
4. **Upgrade hook.** Each installed/replaced coordinator's upgrade hook is
   invoked (see [init and the upgrade hook](#init-and-the-upgrade-hook)).

Finally, any **new role** in the bundle registers its DNA and instantiates its
cell, as at install; any role present only in the installed app is handled by the
same strict-state policy applied to roles (removed), subject to the same
migration/seam rules.

### Relationship to `update_coordinators`

`update_coordinators(dna_hash, coordinator_bundle)` is retained as the **low-level
per-DNA primitive**. `update_app` orchestrates on top of it: it resolves roles to
DNAs, applies strict-state reconciliation and capability handling, and calls the
primitive to actually swap coordinator WASM. Direct use of the primitive remains
available for advanced/scripted flows but is not the app-level path.

## Coordinators and capabilities

Swapping coordinator code can change which zome functions exist, so capability
grants must have a defined lifecycle across an upgrade. This design **binds a
grant to the coordinator it was created under**, with the coordinator hash
**injected by the system** rather than written by the app.

- A grant is created referencing `(zome_name, function_name[])` as today. The
  system records the hash of the coordinator currently installed under that
  name at creation time: effectively `(coordinator_hash, zome_name,
  function_name[])`. App code neither writes nor needs to know the hash.
- On a coordinator upgrade, grants created under the old coordinator hash **do
  not automatically transfer**. They remain present but point at a coordinator
  that is no longer installed, so the capability check will not honour them.
- To carry a capability forward, the new coordinator **re-creates** the grants it
  wants during its upgrade hook (or at any later time); re-creation stamps them
  with the new coordinator hash. Grant creation is **idempotent**, so a hook can
  safely (re-)create grants without tracking whether a prior version already did.
- A capability check honours a grant only if the grant's coordinator hash matches
  the coordinator currently installed under that name. Grants pointing at
  non-extant coordinators are inert (present but never satisfied).

This resolves the older draft's self-contradiction (explicit `version_hash` in
the call path *vs.* system-injected hash): the hash is **system-injected**, and
does not appear in the call path or in app-authored grant/claim code.

### Remote calls and claims

- **Remote calls** may optionally specify the coordinator hash to target, for
  callers that need to pin a specific coordinator version; by default a call
  routes to the current coordinator under the named zome. This addresses the
  "zome calls routed to the wrong hApp/coordinator" class of bug
  ([holochain/holochain#2145](https://github.com/holochain/holochain/issues/2145)).
- **Cap claims** are held by the caller and are matched against the grantor's
  current coordinator, not pinned by the claim holder to a grantor hash they
  cannot know. (The older draft's "claims refer to coordinator hashes" is
  dropped as unworkable.)

## init and the upgrade hook

Two distinct lifecycle points, kept separate so this design stays consistent with
chain continuation's init model:

- **`init` is unchanged and genesis-only.** `init` runs once, at genesis, and
  does **not** re-run on a coordinator upgrade or on a DNA migration. Chain
  continuation depends on this (it does not re-genesis); coordinator upgrades must
  not change it either. So a coordinator upgrade does **not** re-run `init` and
  does not disturb `InitZomesComplete` semantics.
- **A new upgrade hook** runs when a coordinator is installed or replaced by
  `update_app`. It is the defined place for a coordinator to do post-upgrade
  setup — most importantly, to **re-create the capability grants** it wants to
  carry forward (see above). Because grant creation is idempotent, a fresh
  install and an upgrade can run the same hook code.

The upgrade hook is deliberately separate from `init` (which continuation keeps
genesis-only) rather than overloading `init` to "run on install too" as the older
draft proposed. The single hook serves both concerns that need a post-upgrade
moment: coordinator-upgrade capability setup, and — once chain continuation lands
— seeding any content a new integrity version needs, which continuation left as
manual app work.

## Combined DNA + coordinator update

The target workflow — an app release that changes both — is one `update_app` call:

1. The developer builds a new app bundle whose role carries a new coordinator set
   and (optionally) a new integrity version.
2. `update_app` reconciles each role: an integrity change is passed to chain
   continuation (a `ContinueChain` marker is committed, the new integrity version
   is loaded and recorded in the lineage), and the coordinator set is
   strict-state reconciled (replace/install/remove).
3. The upgrade hook runs: grants are re-created for the new coordinators, and any
   new-version seeding is performed.

Shipping coordinator upgrades first means step 2's integrity branch returns
`MigrationNotSupported` for now; the coordinator branch is fully functional
standalone. When continuation lands, the same `update_app` surface starts
carrying integrity changes with no change to how developers invoke it.

## Offline friendliness

A coordinator upgrade is a local operation: loading new coordinator WASM,
reconciling the coordinator set, and running the upgrade hook require no network.
Existing data remains readable and its validation is unaffected (integrity code
did not change). This satisfies the project's offline-friendly principle.

## Open questions and follow-ups

- **Multi-integrity-zome compatibility — resolved, kept.** Verified that the
  current system namespaces types per integrity zome and dispatches validation
  correctly, and that ~half of scanned `lightningrodlabs` / `holochain-open-dev`
  DNAs (23 of 46) declare more than one integrity zome. See
  [decision 1](#scope-and-simplifying-decisions). If dropping multi-zome support
  is ever pursued as a deliberate breaking simplification, it needs a deprecation
  path (e.g. splitting shared zomes like `profiles_integrity` into their own
  DNAs), not a silent removal.
- **Side-by-side coordinator sets sharing a DNA.** The older draft explored two
  or more coordinator sets over one DNA; this design assumes a single set per
  role. Multi-set sharing is deferred.
- **`UseExisting` / cell reuse.** Cross-app cell reuse and the capability
  questions it raises (how one app grants another capability over a shared cell)
  are out of scope here and left to the migration/`UseExisting` work. Note the
  current `CellProvisioning::UseExisting` is already deprecated in favour of
  updating coordinators for late binding and bridge calls for cross-app.
- **Capability detail.** The grant/claim matching rules above are the model, not
  a wire format; exact storage of the injected coordinator hash and the
  claim-side matching path need a detailed pass.
- **Package-management dependency graph.** The older draft's UI→coordinator→
  integrity dependency-graph and lockfile ideas are a packaging concern and are
  not part of this runtime design; recorded as a possible later addition.

## Non-goals

- This document designs coordinator upgrades only. It does not design chain
  continuation or chain switch; it only defines the seam through which an
  integrity change is delegated to them.
- It does not make upgrades automatic or developer-forced. As with migration,
  applying an app update is the user's choice.
