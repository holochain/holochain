# Coordinator Upgrades Design

## Status

**Draft / proposed.** This document describes **coordinator upgrades**: replacing
the coordinator code of an installed app with a new version, at the app level,
without touching the integrity rules, the network, or the source chain.

It modernises an older "Update Coordinators" spec that predated large parts of
the current system (the unified per-DNA database, the v2 model types, and the
[chain continuation](./dna_migration_chain_continuation.md) migration design).

Coordinator upgrades are designed to **compose with, but not depend on**, chain
continuation. They are intended to ship first. An app release will often carry
*both* a new coordinator set and a new integrity version, so `update_app` leaves
a clean **seam** for an integrity-version change to be delegated to chain
continuation rather than forbidding it. Applying an app update is always the
user's choice, never automatic or developer-forced.

## Terminology

- An **integrity zome** defines entry/link types and the validation rules that
  govern them. Integrity code is what is validated against, and it contributes to
  the `DnaHash` (along with the network seed and properties). Changing it is a
  *migration*, not a coordinator upgrade.
- A **coordinator zome** exposes the callable zome functions an app and its UI
  use. Coordinators hold no validation authority and do not affect the
  `DnaHash`; they may be replaced freely, including with coordinators from a
  different publisher. Replacing them does, however, change the zome's public API
  as seen from outside Holochain.
- A **coordinator set** is the collection of coordinator zomes bound to one role
  of an installed app. A cell runs exactly **one** coordinator set at a time —
  the latest installed.
- A **coordinator upgrade** replaces one or more of an installed app's coordinator
  sets — an app may have several roles, each with its own set — with new ones.
  Integrity code, network, agent key, and source chain are untouched.

## Motivation

Coordinator code is where an app's behaviour lives and where most iteration
happens: new zome functions, bug fixes, changed call surfaces. None of that
should require a new network, a new chain, or re-validation of existing data —
coordinators do not validate and do not affect the `DnaHash`.

The current path is `update_coordinators(dna_hash, coordinator_bundle)`
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

This design adds an app-level `update_app` as the public path for upgrading an
app, and defines the coordinator and capability lifecycle across an upgrade.
`update_coordinators` leaves the public admin API; any equivalent per-DNA
operation `update_app` needs becomes an internal detail.

## Design decisions

These are the changes this design makes to the manifests and install model.
Behaviour it leaves untouched — multiple integrity zomes per DNA, and the
two-layer `network_seed`/`properties` defaults overridable per role — is not
restated here.

1. **Coordinators are bound to the app, not the DNA.** The coordinator set is
   declared per role in the app manifest, not inside the DNA bundle. This makes a
   coordinator a swappable, app-owned component and is the precondition for
   app-level `update_app`.
2. **`clone_limit` is a role field.** It is a provisioning concern, not a DNA
   property, and lives on the role.
3. **The coordinator declaration is a flat list.** With nothing else expected
   beside the zome list, the coordinator declaration is the list itself rather
   than a `zomes:`-nested object.

## Manifest changes

### DNA manifest — coordinators move out

The DNA manifest loses coordinators (moved to the app). It keeps its
`network_seed`/`properties` defaults and its integrity zome list — a DNA may
still declare several integrity zomes.

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
manifest_version: "0"                  # version left at 0; not stabilised here
name: group
integrity:            # seed/properties kept as defaults; only coordinators removed
  network_seed: 00000000-0000-0000-0000-000000000000
  properties: ~
  zomes:              # a DNA may have several integrity zomes
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
      modifiers:
        network_seed: ~
        properties: ~
      installed_hash: ~
      clone_limit: 5
    # coordinators live inside the DNA bundle, not here
```

After (proposed):

```yaml
manifest_version: "0"                  # version left at 0; not stabilised here
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
  author has to compute by hand.
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
no partial "add these coordinators" mode. Strict state means the reconciliation
has a single, well-defined outcome, and orphaned coordinators — present in the
installation but absent from the new bundle — are removed rather than left
dangling.

### Reconciliation algorithm

`update_app` **validates the whole update before changing anything**. It runs in
two phases: phase 1 changes nothing, so it cannot leave the app partly updated,
and phase 2 is applied atomically, so a failure mid-apply rolls back to the
pre-upgrade state.

**Phase 1 — validate every role, apply nothing.** For each role in the bundle,
compute the plan without touching the installation:

- **DNA identity.** Compare the role's effective DNA identity — the `DnaHash`,
  which is integrity code together with the network seed and properties — against
  the installed one. Any difference (changed integrity, seed, or properties) means
  the role needs a *migration*; until chain continuation ships this makes the whole
  update fail with `MigrationNotSupported` naming the role. `update_app` never
  silently applies a coordinator update to a role whose DNA identity moved.
- **Coordinators.** Diff the installed coordinator set against the bundle's set
  for this role, by coordinator name, into replace/install/remove actions.

If *any* role fails validation, `update_app` returns an error and the
installation is untouched — no partial update.

**Phase 2 — apply.** Only once every role validates, apply the computed plans:

1. **Coordinators (strict).** For each role, by coordinator name:
   - Present in both → **replace** the coordinator WASM under that name.
   - In the bundle only → **install** the new coordinator.
   - Installed only, absent from the bundle → **remove** it.
2. **Capabilities.** Coordinator changes are followed by the capability
   reconciliation described in [Coordinators and
   capabilities](#coordinators-and-capabilities).
3. **New / removed roles.** A **new role** is provisioned per its manifest
   strategy, exactly as at install: an immediate role registers its DNA and
   instantiates its cell, while a `provisioning.deferred` role registers its DNA
   but is not instantiated until deferred provisioning. A role present only in the
   installed app is removed under the same strict-state policy.

Phase 2 is applied as one unit. The coordinator, capability, and DNA-registry
writes commit in a single transaction; the one step with external side effects —
instantiating a new role's cell — is ordered last and, if any part of the apply
fails, the freshly registered cell is torn down so the installation is left on
its pre-upgrade state.

The upgrade hook of an installed/replaced coordinator is not run inline here; it
is invoked lazily (see [The upgrade hook](#the-upgrade-hook)).

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
  wants in its `init_upgrade` hook (or at any later time); re-creation stamps them
  with the new coordinator hash. Grant creation is **idempotent**, so a hook can
  safely (re-)create grants without tracking whether a prior version already did.
- A capability check honours a grant only if the grant's coordinator hash matches
  the coordinator currently installed under that name. Grants pointing at
  non-extant coordinators are inert (present but never satisfied).
- Grants committed **before this design** carry no recorded coordinator hash.
  They keep working via legacy matching on `(zome_name, function_name[])` alone;
  the hash check applies only to grants stamped under this design. A coordinator
  that wants a legacy grant bound to its hash re-creates it, as above.

### Remote calls and claims

- **Remote calls** route to the coordinator **currently** installed under the
  named zome; a cell runs one coordinator set, so there is no older version to
  target. A caller may pass the expected coordinator hash as a **guard** — the
  call proceeds only if it matches the installed hash and is rejected otherwise,
  rather than silently running a different coordinator. This addresses the "zome
  calls routed to the wrong hApp/coordinator" class of bug
  ([holochain/holochain#2145](https://github.com/holochain/holochain/issues/2145)).
- **Cap claims** are held by the caller and are matched against the grantor's
  current coordinator, not pinned by the claim holder to a grantor hash they
  cannot know.

### Storage and matching

The coordinator hash is stored **on the grant**: a `CapGrant` is committed as
today, and the system records the hash of the coordinator installed under the
grant's zome name as an additional field at commit time (not part of the
app-authored payload). At check time the capability path resolves the hash of the
coordinator *currently* installed under that zome name and admits the grant only
when the two match. A claim carries only the secret/assignee it does today; the
grantor-side match is what pins it to a live coordinator, so no coordinator hash
is ever written by claim-side app code.

## The upgrade hook

A coordinator needs a defined moment to do post-upgrade setup — most importantly,
to **re-create the capability grants** it wants to carry forward (see above). This
design introduces an optional coordinator callback, **`init_upgrade`** (name
provisional), for that purpose. It mirrors `init`'s invocation model:

- **`init` runs once, lazily.** It is not run eagerly at genesis; it is invoked
  on the first zome call, after genesis but **before any other zome function is
  allowed to run**. It does not re-run on a coordinator upgrade and must not
  disturb `InitZomesComplete` semantics.
- **`init_upgrade` runs lazily after an upgrade.** `update_app` does not call it
  inline; it is invoked on the first zome call following the upgrade, before any
  other zome function of the new coordinator runs — the same "must complete first"
  ordering `init` has. Because grant creation is idempotent, a fresh install and
  an upgrade can run the same setup code.

Like `init`, `init_upgrade` has network access available to it. Whether the hook
does any networked work — and therefore whether an upgrade stays fully offline —
is up to the app developer.

## Combined DNA + coordinator update

The target workflow — an app release that changes both — is one `update_app` call:

1. The developer builds a new app bundle whose role carries a new coordinator set
   and (optionally) a new integrity version.
2. `update_app` reconciles each role: a DNA-identity change (integrity, seed, or
   properties) is delegated to the migration path, and the coordinator set is
   strict-state reconciled (replace/install/remove).
3. On the next zome call, the new coordinators' `init_upgrade` hook runs lazily
   and re-creates the grants they carry forward.

Shipping coordinator upgrades first means step 2's integrity branch returns
`MigrationNotSupported` for now; the coordinator branch is fully functional
standalone. When continuation lands, the same `update_app` surface starts
carrying integrity changes with no change to how developers invoke it.

## Offline friendliness

The `update_app` operation is local: loading new coordinator WASM and reconciling
the coordinator set require no network. Existing data remains readable and its
validation is unaffected, since integrity code did not change. The `init_upgrade`
hook that runs afterwards has network access (like `init`), so any networked work
an app chooses to do there — and therefore whether an upgrade stays fully offline
— is the app developer's responsibility.

## Open questions and follow-ups

- **Multiple integrity-zome dependencies per coordinator.** The manifest
  `dependencies` field is a list, but the platform currently supports **at most
  one** integrity-zome dependency per coordinator. A coordinator that needs types
  from several integrity zomes in one DNA (e.g. `group` plus `profiles`) is not
  expressible today. Whether to lift this limit is an open question.
- **`UseExisting` / cell reuse.** Cross-app cell reuse and the capability
  questions it raises (how one app grants another capability over a shared cell)
  are out of scope here and left to the migration/`UseExisting` work. Note the
  current `CellProvisioning::UseExisting` is already deprecated in favour of
  updating coordinators for late binding and bridge calls for cross-app.
