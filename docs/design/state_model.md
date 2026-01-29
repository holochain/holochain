# Data Logic Design Reference
# Data State Model

## Overview

The Holochain data storage and validation architecture provides:

1. [Action and entry](./data_model.md) data storage for agents' authored chains
1. Ops as the unit of validation, with an aggregated validity status for records
2. Validation limbo tables to separate pending data from validated DHT data
3. Unified data querying without separate cache database
4. Distinct schemas for authored and DHT databases, with limbo tables in DHT
5. Direct data queries without complex joins

## Architecture

### Core Principles

1. **Ops are the unit of validation**: All validation happens at the op level
2. **Records aggregate op validity**: A record's validity is derived from its constituent ops
3. **Validation limbo isolates pending data**: Unvalidated ops stay in limbo tables until validated
4. **Distinct schemas per database type**: Authored and DHT databases have schemas tailored to their needs
5. **Unified data storage**: DHT database serves both obligated and cached data, distinguished by arc coverage
6. **Clear state transitions**: Data moves through well-defined states with no ambiguity

### Database Structure

#### 1. Authored Database
**Purpose**: Store an agent's own authored chain data.

Regardless of what data the agent stores and validates on behalf of other DHT agents, their own authored chain is 
always fully stored and accessible.

```sql
-- Authored actions
CREATE TABLE Action (
    hash         BLOB PRIMARY KEY,
    author       BLOB NOT NULL,
    seq          INTEGER NOT NULL,
    prev_hash    BLOB,
    timestamp    INTEGER NOT NULL,
    action_type  TEXT NOT NULL,
    action_data  BLOB,         -- Serialized ActionData enum, containing action-type fields
   
    -- Reference fields for entry meta
    entry_hash   BLOB,         -- NULL for non-entry actions
);

-- Authored entries
CREATE TABLE Entry (
    hash BLOB PRIMARY KEY,
    blob BLOB NOT NULL,
);

-- Capability grants lookup table.
-- 
-- For simpler querying of cap grants from the agent chain.
CREATE TABLE CapGrant (
    action_hash BLOB PRIMARY KEY,
    cap_access  TEXT NOT NULL, -- 'unrestricted', 'transferable', 'assigned'
    tag         TEXT,
   
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- Capability claims table.
-- 
-- For recording cap claims from other agents that are granted to a local agent.
CREATE TABLE CapClaim (
    action_hash BLOB PRIMARY KEY,
    tag         TEXT NOT NULL,
    grantor     BLOB NOT NULL,
    secret      BLOB NOT NULL,
);

-- Authored ops.
-- 
-- For publishing state only, does not contain a complete op.
CREATE TABLE AuthoredOp (
    hash        BLOB PRIMARY KEY,
    action_hash BLOB NOT NULL,
    op_type     TEXT NOT NULL,
    basis_hash  BLOB NOT NULL,
    
    -- Publishing state
    last_publish_time INTEGER,
    receipts_complete BOOLEAN,
    
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- Validation receipts for authored ops.
--
-- For tracking that other agents have validated our authored ops
CREATE TABLE ValidationReceipt (
    hash          BLOB PRIMARY KEY,
    op_hash       BLOB NOT NULL,
    validators    BLOB NOT NULL,
    signature     BLOB NOT NULL,
    when_received INTEGER NOT NULL,
    
    FOREIGN KEY(op_hash) REFERENCES AuthoredOp(hash)
);
```

#### 2. DHT Database
**Purpose**: Store validated DHT data, and ops pending validation

```sql
-- DHT actions.
CREATE TABLE DhtAction (
   hash          BLOB PRIMARY KEY,
   author        BLOB NOT NULL,
   seq           INTEGER,      -- NULL for actions not in agent activity
   prev_hash     BLOB,         -- NULL for actions not in agent activity
   timestamp     INTEGER NOT NULL,
   action_type   TEXT NOT NULL,
   action_data   BLOB NOT NULL, -- Serialized ActionData enum

   -- Reference fields for entry meta
   entry_hash    BLOB,         -- NULL for non-entry actions

   -- Record validity (aggregated from all ops for this record)
   -- A record is the combination of action + entry (if applicable)
   record_validity TEXT NOT NULL -- 'valid', 'rejected'
);

-- DHT entries.
CREATE TABLE DhtEntry (
   hash BLOB PRIMARY KEY,
   blob BLOB NOT NULL
);

-- Limbo for DHT ops which are in the process of being validated.
CREATE TABLE LimboOp (
    hash        BLOB PRIMARY KEY,
    op_type     TEXT NOT NULL,
    action_hash BLOB NOT NULL,
    
    -- DHT location
    basis_hash         BLOB NOT NULL,
    storage_center_loc INTEGER NOT NULL,
    
    -- Local validation state
    validation_stage      TEXT NOT NULL, -- 'pending_sys', 'pending_app', 'complete'
    sys_validation_status TEXT,          -- NULL, 'valid', 'rejected', 'abandoned'
    app_validation_status TEXT,          -- NULL, 'valid', 'rejected', 'abandoned'
    
    -- Timing
    when_received INTEGER NOT NULL,
    validation_attempts INTEGER DEFAULT 0,
    last_validation_attempt INTEGER,
    
    -- The action referenced by this op
    action_blob BLOB NOT NULL,
);

-- DHT ops which have completed validation and are integrated into the DHT.
CREATE TABLE DhtOp (
    hash        BLOB PRIMARY KEY,
    op_type     TEXT NOT NULL,
    action_hash BLOB NOT NULL,

    -- DHT location
    basis_hash         BLOB NOT NULL,
    storage_center_loc INTEGER NOT NULL,

    -- Final validation result
    validation_status TEXT NOT NULL,    -- 'valid', 'rejected'
    locally_validated BOOLEAN NOT NULL, -- whether this op validated by us, or fetched from an authority

    -- Timing
    when_received   INTEGER NOT NULL, -- copied from LimboOp
    when_integrated INTEGER NOT NULL, -- set when moved from LimboOp

    FOREIGN KEY(action_hash) REFERENCES DhtAction(hash)
);
```

### Rust Structure

Actions:

```rust
/// Common action header stored for all action types
pub struct ActionHeader {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: ActionHash,
}

/// Action-specific data stored separately from header
pub enum ActionData {
    Dna(DnaData),
    AgentValidationPkg(AgentValidationPkgData),
    InitZomesComplete(InitZomesCompleteData),
    Create(CreateData),
    Update(UpdateData),
    Delete(DeleteData),
    CreateLink(CreateLinkData),
    DeleteLink(DeleteLinkData),
}

/// Full action with data loaded on-demand
pub struct Action {
    pub hash: ActionHash,
    pub header: ActionHeader,
    pub data: ActionData,
}

// Action-specific data structures (without redundant common fields)
pub struct CreateData {
    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
    pub weight: EntryRateWeight,
}

pub struct UpdateData {
    pub original_action_address: ActionHash,
    pub original_entry_address: EntryHash,
    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
    pub weight: EntryRateWeight,
}

pub struct DeleteData {
    pub deletes_address: ActionHash,
    pub deletes_entry_address: EntryHash,
    pub weight: RateWeight,
}

pub struct CreateLinkData {
    pub base_address: AnyLinkableHash,
    pub target_address: AnyLinkableHash,
    pub zome_index: ZomeIndex,
    pub link_type: LinkType,
    pub tag: LinkTag,
    pub weight: RateWeight,
}

pub struct DeleteLinkData {
    pub base_address: AnyLinkableHash,
    pub link_add_address: ActionHash,
}

// Minimal data for chain-only actions
pub struct DnaData {
    pub dna_hash: DnaHash,
}

pub struct AgentValidationPkgData {
    pub membrane_proof: Option<MembraneProof>,
}

pub struct InitZomesCompleteData {}
```

DHT Operations:

```rust
/// Top-level DHT operation that can be either a chain operation or a warrant.
pub enum DhtOp {
    /// An op representing storage of some record information.
    ChainOp(Box<ChainOp>),
    /// An op representing storage of a claim that a ChainOp was invalid.
    WarrantOp(Box<WarrantOp>),
}

/// Chain operations that represent chain data distributed across the network.
/// Each operation is stored at a specific DHT location determined by its basis hash.
pub enum ChainOp {
    /// Stores the complete record at the record authority
    CreateRecord(SignedAction, Option<Entry>),
    /// Stores entry content at the entry authority
    CreateEntry(SignedAction, Entry),
    /// Agent activity stored at the agent's authority
    AgentActivity(SignedAction),
    /// Entry updates indexed at the original entry authority
    UpdateEntry(SignedAction, Entry),
    /// Updates indexed at the original record authority
    UpdateRecord(SignedAction),
    /// Entry deletes indexed at the original entry authority
    DeleteEntry(SignedAction),
    /// Deletes indexed at the original record authority
    DeleteRecord(SignedAction),
    /// Links indexed at the base address
    CreateLink(SignedAction),
    /// Link deletes indexed at the base address
    DeleteLink(SignedAction),
}

/// Warrant operation representing a claim that a ChainOp was invalid.
pub struct WarrantOp(SignedWarrant);
```

### Creation and Distribution Flow

The high-level flow for authoring actions and distributing ops is as follows:

```
1. Self validation
   ├─> Run sys validation check
   ├─> Run app validation checks via WASM
   └─> On failure: rollback transaction, chain unchanged

2. Author new action locally
   ├─> Insert into `Action` table
   ├─> Insert into `Entry` table (if applicable)
   └─> Insert into `CapGrant` table (if applicable)

3. Create ops for publishing
   ├─> If this is a countersigning action, skip this step (ops created on session completion)
   └─> Insert into `AuthoredOp` with publishing state

4. Publish DHT Ops Workflow
   ├─> Query ops which are ready for publishing from `AuthoredOp`
   ├─> Group by `basis_hash` for efficient sending
   ├─> Send ops to DHT authorities over the network
   └─> Update `last_publish_time` in `AuthoredOp`

5. Validation Receipt Workflow
   ├─> Receive validation receipts from validators
   ├─> Insert into `ValidationReceipt` table
   └─> Update `receipts_complete` in `AuthoredOp` when sufficient receipts received

6. Countersigning Session Completion Workflow (if applicable)
   ├─> Verify all participants have signed
   ├─> Create ops from the countersigned action/entry
   ├─> Insert into `AuthoredOp` with publishing state
   ├─> Unlock the chain
   └─> Trigger publish workflow
```

#### Publish Query and Op Construction

The publish workflow queries the authored database for ops that need publishing and constructs the full `ChainOp` (wrapped in `DhtOp`) for network transmission.

**Query Logic:**

The query selects from `AuthoredOp` table, joining to `Action` and optionally `Entry`:

```sql
SELECT
    Action.hash as action_hash,
    Action.author,
    Action.timestamp,
    Action.action_type,
    Action.action_data,
    Entry.blob as entry_blob,
    AuthoredOp.hash as op_hash,
    AuthoredOp.op_type,
    AuthoredOp.basis_hash
FROM AuthoredOp
JOIN Action ON AuthoredOp.action_hash = Action.hash
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE
    Action.author = :author
    AND (AuthoredOp.last_publish_time IS NULL
        OR AuthoredOp.last_publish_time <= :recency_threshold)
    AND AuthoredOp.receipts_complete IS NULL
ORDER BY Action.seq, Action.timestamp
```

**In-Memory Transform:**

For each row returned:
1. **Deserialize Action**: Reconstruct full `Action` from common fields + the `action_data` BLOB
2. **Construct ChainOp**: Build the appropriate `ChainOp` variant based on `op_type`:
   - `CreateRecord`: Requires SignedAction + Entry (if present)
   - `CreateEntry`: Requires SignedAction + Entry
   - `AgentActivity`: Requires SignedAction only
   - `UpdateEntry`: Requires SignedAction + Entry
   - `UpdateRecord`: Requires SignedAction only
   - `DeleteEntry`: Requires SignedAction only
   - `DeleteRecord`: Requires SignedAction only
   - `CreateLink`: Requires SignedAction only
   - `DeleteLink`: Requires SignedAction only
3. **Wrap in DhtOp**: Wrap the `ChainOp` in `DhtOp::ChainOp(Box::new(chain_op))`
4. **Group by Basis**: Collect ops by `basis_hash` for efficient network transmission

**Differences to Current Implementation:**

1. **Missing `withhold_publish` field**: The current code checks `DhtOp.withhold_publish IS NULL` to exclude countersigning ops. The new `AuthoredOp` schema doesn't include this field.
   - **Resolution**: Countersigning completion should produce ops from the action/entry instead of clearing a field. No `withhold_publish` field needed in `AuthoredOp` table. During the countersigning session, ops are not created at all - they are only created when the session successfully completes, at which point they are immediately publishable. This approach is superior because: (a) entries should not be served during active countersigning sessions since the session may fail, (b) it eliminates intermediate "withheld" state and associated cleanup complexity, (c) op existence semantically means "publishable data", and (d) failed sessions leave no garbage ops in the database.

2. **Missing `when_integrated` field**: The current code checks `DhtOp.when_integrated IS NOT NULL` to ensure ops are only published after local validation completes. The new `AuthoredOp` schema doesn't track integration.
   - *Resolution needed*: Authored ops don't need integration tracking since they're locally validated before insertion. This filter may be unnecessary in the new model, or we need a different field like `locally_validated BOOLEAN`.

3. **`op_order` field**: Not needed in the new design.
   - **Current Purpose**: `OpOrder` combines op type priority (0-9) with timestamp to ensure "the most likely ordering where dependencies will come first"
   - **New Approach**: Use `ORDER BY Action.seq, Action.timestamp` in the publish query
   - **Rationale**:
     - Sequence number naturally orders actions from earliest to latest in the chain
     - Publishing foundational data first helps nodes joining large networks validate more efficiently
     - Timestamp breaks ties when multiple ops reference the same action
     - Simpler than maintaining a computed ordering field
   - **Resolution**: No `op_order` column needed in `AuthoredOp`

4**Warrant Ops**: The current query includes a UNION for Warrant ops from a separate `Warrant` table. The new design doesn't show warrant storage.
   - *Resolution needed*: Clarify where warrants are stored (likely in Action table with `action_type = 'Warrant'`).

### Validation Flow

```
1. Op arrives (from author or network)
   └─> Insert into LimboOp

2. Sys Validation Workflow
   ├─> Check dependencies in DhtAction/DhtEntry
   ├─> Perform sys validation checks
   └─> Update sys_validation_status in LimboOp

3. App Validation Workflow (if sys valid)
   ├─> Run WASM validation
   └─> Update app_validation_status in LimboOp

4. Integration Workflow (if both valid)
   ├─> Move op from LimboOp to DhtOp
   ├─> Insert/update DhtAction with aggregated validity
   ├─> Insert DhtEntry (if applicable and not already present)
   └─> Delete from LimboOp
```

### Record Validity Aggregation

**Rules:**
1. A record is **INVALID** if ANY known ops for it are rejected
2. A record is **VALID** if at least one known op for it is valid and no known ops are rejected
3. Abandoned ops never leave the limbo state, so records in DHT only have 'valid' or 'rejected' status
4. A record's validity is computed on integration, not on query
5. Validators may have partial views due to sharding - validity is based only on known ops

The record validity is determined at the time of integration by examining all known ops associated with the record. Due to the sharding model, a validator may not have all ops for a record - they hold specific op types over specific ranges. This aggregated status is stored with the record itself, eliminating the need for complex joins during queries.

**Note on Partial Views:**
Since validators hold shards (specific op types over specific ranges), they make validity decisions based on the ops they know about. A record is considered valid if any of its known ops are valid and none are known to be invalid. The absence of some ops does not make a record pending or invalid.

### Record Validity Correction

TODO: It is a future piece of work to define this logic.

## Query Patterns

### Get Record by Hash

```sql
SELECT Action.*, Entry.*
FROM DhtAction AS Action
LEFT JOIN DhtEntry AS Entry ON Action.entry_hash = Entry.hash
WHERE Action.hash = ?
  AND Action.record_validity = 'valid'
```

### Get Entry by Hash

```sql
-- Entry authorities always have the action, so we can check validity
SELECT Entry.*, Action.record_validity
FROM DhtEntry AS Entry
JOIN DhtAction AS Action ON Action.entry_hash = Entry.hash
WHERE hash = ?
  AND Action.record_validity = 'valid'
```

### Agent Activity

```sql
SELECT * FROM DhtAction
WHERE author = ?
  AND record_validity = 'valid'
ORDER BY seq
```

## Workflow Specifications

### Incoming DHT Ops Workflow
Ops are inserted into LimboOp table with validation_stage='pending_sys'

### Sys Validation Workflow  
Updates sys_validation_status in LimboOp, triggers app validation or abandonment

### App Validation Workflow
Updates app_validation_status in LimboOp, triggers integration if valid

### Integration Workflow
Moves validated ops from LimboOp to DhtOp, updates record validity in DhtAction

### Publish DHT Ops Workflow
Queries AuthoredOp from the authored database for unpublished or recently published ops, tracking publish attempts and timing

### Validation Receipt Workflow
Tracks validation receipts for authored ops. When sufficient receipts are received for an authored op, updates receipts_complete flag in AuthoredOp table to prevent unnecessary republishing

## Unified Storage with Arc Coverage

The DHT database stores all validated data, with arc coverage determining storage obligations. This eliminates the need for a separate cache database and the associated query complexity of merging results from two sources.

**Key aspects:**
1. **Single query path**: All data queries go to the DHT database
2. **Arc coverage tracking**: Determines which data is obligated vs cached
3. **Retention policy**: Data outside arc can be pruned under storage pressure
4. **No result merging**: Eliminates complex logic combining DHT and cache results

```sql
-- Arc coverage table (in conductor database)
CREATE TABLE ArcCoverage (
    dna_hash BLOB NOT NULL,
    agent_hash BLOB NOT NULL,
    arc_start INTEGER NOT NULL,  -- Start of arc range (0-2^32)
    arc_end INTEGER NOT NULL,    -- End of arc range (0-2^32)
    last_updated INTEGER NOT NULL,
    
    PRIMARY KEY (dna_hash, agent_hash)
);
```

Data classification:
- **Within arc**: Obligated storage (storage_center_loc in [arc_start, arc_end])
- **Outside arc**: Cached data, eligible for pruning
- **Authored**: Always retained regardless of arc

### Cache Pruning Strategy

When storage pressure occurs:
1. Identify ops outside current arc (storage_center_loc not in [arc_start, arc_end])
2. Sort by last_access_time (tracked separately)
3. Prune oldest first until storage target met
4. Never prune authored ops or ops within arc
5. Consider keeping frequently accessed data even if outside arc

## Warrant Handling

Warrants require special consideration:

1. **ChainIntegrityWarrant**: Proves an author broke chain rules
   - Stays in LimboOp until warranted action is fetched and validated
   - If warranted action is rejected, warrant moves to DhtOp as valid
   - If warranted action is valid, warrant is rejected

2. **Warrant Validation Dependencies**: 
   - Warrants can have up to 2 dependencies (the actions being warranted)
   - These must be resolved before warrant validation can complete
   - Dependencies tracked in dependency1 and dependency2 fields of LimboOp

## Key Operations

### Mutations

1. **insert_network_op** (from network)
   - Insert into LimboOp with validation_stage='pending_sys'

2. **author_action** (local authoring)
   - Insert into authored Action/Entry tables
   - Create AuthoredOp records
   - Validate locally before committing
   - On validation failure, rollback without affecting chain

3. **publish_authored_op** (after local validation)
   - Mark AuthoredOp as published (set when_published)
   - Op enters network propagation

4. **set_validation_status** (after validation)
   - Update LimboOp.sys/app_validation_status

5. **set_when_integrated** (after all validation)
   - Move from LimboOp to DhtOp
   - Update record validity in DhtAction
   - Insert DhtEntry if applicable

6. **set_receipts_complete** (after enough receipts)
   - Update DhtOp.receipts_complete

### Queries

1. **get_record** - Query DhtAction + DhtEntry by record_validity
2. **get_entry** - Query DhtEntry joined with DhtAction for record_validity
3. **get_links** - Direct query of DhtLink table
4. **agent_activity** - Query DhtAction by author and record_validity
5. **validation_limbo** - Query LimboOp by validation_stage

## Data Integrity Invariants

The system maintains these invariants:

1. **No op exists in both LimboOp and DhtOp simultaneously**
2. **Every DhtOp has a definite validation_status (never NULL)**
3. **Every DhtAction has a computed record_validity**
4. **Ops move from limbo to DHT atomically with record updates**
5. **Rejected ops in DhtOp cause their records to be marked invalid**
6. **Dependencies are resolved before validation proceeds**
7. **Authored ops remain in authored database until locally validated**
8. **Failed local validation can be rolled back without affecting the chain**

## Optimized Action Storage Design

Actions are stored with common fields in the main table and action-specific data as a serialized BLOB. Where action-specific fields need to be queried, separate index tables provide efficient access.

### Database Schema

```sql
-- Main action storage
CREATE TABLE Action (
    hash          BLOB PRIMARY KEY,
    author        BLOB NOT NULL,
    seq           INTEGER NOT NULL,
    prev_hash     BLOB,
    timestamp     INTEGER NOT NULL,
    action_type   TEXT NOT NULL,
    action_data   BLOB,         -- Serialized ActionData enum
    entry_hash    BLOB,         -- NULL for non-entry actions
    private_entry BOOLEAN       -- NULL for non-entry actions, TRUE/FALSE for entry actions
);
```

Note: Queryable entry types (CapGrant, CapClaim) have dedicated tables in the Entry section for direct lookup without requiring full chain scans.



### Query Patterns

Cap grant/claim lookups use dedicated tables for direct access without chain scans:

```sql
-- Find cap grant by access type (direct lookup, join for other fields)
SELECT cg.*, Action.*, Entry.*
FROM CapGrant cg
JOIN Action ON cg.action_hash = Action.hash
JOIN Entry ON Action.entry_hash = Entry.hash
WHERE cg.cap_access = ?;

-- Find all grants by an author (join to Action for author)
SELECT cg.*, Action.*, Entry.*
FROM CapGrant cg
JOIN Action ON cg.action_hash = Action.hash
JOIN Entry ON Action.entry_hash = Entry.hash
WHERE Action.author = ?;

-- Find grants by tag
SELECT cg.*, Action.*, Entry.*
FROM CapGrant cg
JOIN Action ON cg.action_hash = Action.hash
JOIN Entry ON Action.entry_hash = Entry.hash
WHERE cg.tag = ?;

-- Find cap claims by grantor (direct lookup)
SELECT cc.*, Action.*, Entry.*
FROM CapClaim cc
JOIN Action ON cc.action_hash = Action.hash
JOIN Entry ON Action.entry_hash = Entry.hash
WHERE cc.grantor = ?;

-- Find cap claims by tag
SELECT cc.*, Action.*, Entry.*
FROM CapClaim cc
JOIN Action ON cc.action_hash = Action.hash
JOIN Entry ON Action.entry_hash = Entry.hash
WHERE cc.tag = ?;

-- Chain traversal (no BLOB deserialization needed)  
SELECT hash, seq, prev_hash, timestamp, action_type
FROM Action
WHERE author = ?
ORDER BY seq;

-- Full action retrieval (deserialize BLOB for details)
SELECT * FROM Action WHERE hash = ?;
-- Then deserialize action_data BLOB in application
```

### Benefits of This Approach

1. **Minimal redundancy** - Common fields stored once
2. **Simple core schema** - Main table remains clean
3. **Selective indexing** - Only queryable fields get index tables
4. **Efficient chain operations** - No BLOB deserialization for traversal
5. **Flexible querying** - Index tables enable specific queries without full scans
