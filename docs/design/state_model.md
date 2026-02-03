# Data Logic Design Reference
# Data State Model

## Overview

The Holochain data storage and validation architecture provides:

1. [Action and entry](./data_model.md) data storage for agents' authored chains
1. Ops as the unit of validation, with an aggregated validity status for records
2. Validation limbo table (`LimboOp`) to track pending ops, with shared DhtAction/DhtEntry tables
3. Unified data querying without separate cache database
4. Distinct schemas for authored and DHT databases
5. Direct data queries without complex joins

## Architecture

### Core Principles

1. **Ops are the unit of validation**: All validation happens at the op level
2. **Records aggregate op validity**: A record's validity is derived from its constituent ops
3. **Validation limbo isolates pending ops**: Unvalidated ops stay in LimboOp table until validated, with actions and entries in shared DhtAction/DhtEntry tables marked by NULL record_validity
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

-- Link index table.
--
-- For efficient link queries. Populated at integration time when CreateLink ops are validated.
CREATE TABLE Link (
    action_hash BLOB PRIMARY KEY,
    base_hash   BLOB NOT NULL,
    zome_index  INTEGER NOT NULL,
    link_type   INTEGER NOT NULL,
    tag         BLOB,

    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- Deleted link index table.
--
-- For tracking which links have been deleted. Populated at integration time when DeleteLink ops are validated.
CREATE TABLE DeletedLink (
    action_hash      BLOB PRIMARY KEY,  -- The DeleteLink action
    create_link_hash BLOB NOT NULL,      -- The CreateLink being deleted

    FOREIGN KEY(action_hash) REFERENCES Action(hash),
    FOREIGN KEY(create_link_hash) REFERENCES Link(action_hash)
);

-- Chain lock table.
--
-- For coordinating countersigning sessions. Prevents new actions from being committed to the chain
-- while a countersigning session is in progress.
CREATE TABLE ChainLock (
    author                BLOB PRIMARY KEY,  -- Agent who holds the lock
    subject               BLOB NOT NULL,      -- What is being locked (e.g., session hash)
    expires_at_timestamp  INTEGER NOT NULL    -- Unix timestamp when lock expires
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

-- Authored warrants.
--
-- For warrants issued by this agent about invalid behavior by other agents
CREATE TABLE Warrant (
    hash      BLOB PRIMARY KEY,
    author    BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    warrantee BLOB NOT NULL,
    proof     BLOB NOT NULL,  -- Serialized WarrantProof (InvalidChainOp or ChainFork)

    -- Publishing state
    last_publish_time INTEGER,
    receipts_complete BOOLEAN
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
   -- NULL for records in limbo, 'valid' or 'rejected' after integration
   record_validity TEXT -- NULL, 'valid', 'rejected'
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

    -- Validation receipt requirement
    require_receipt BOOLEAN NOT NULL,    -- Whether to send validation receipt back to author

    -- Timing and attempt tracking
    when_received INTEGER NOT NULL,
    sys_validation_attempts INTEGER DEFAULT 0,
    app_validation_attempts INTEGER DEFAULT 0,
    last_validation_attempt INTEGER,

    FOREIGN KEY(action_hash) REFERENCES DhtAction(hash)
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

-- DHT warrants.
--
-- For warrants received from the network about invalid behavior
CREATE TABLE DhtWarrant (
    hash      BLOB PRIMARY KEY,
    author    BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    warrantee BLOB NOT NULL,
    proof     BLOB NOT NULL,  -- Serialized WarrantProof (InvalidChainOp or ChainFork)

    -- DHT location (stored at warrantee's agent authority)
    storage_center_loc INTEGER NOT NULL
);

-- Link index table.
--
-- For efficient link queries. Populated at integration time when CreateLink ops are validated.
CREATE TABLE Link (
    action_hash BLOB PRIMARY KEY,
    base_hash   BLOB NOT NULL,
    target_hash BLOB NOT NULL,
    zome_index  INTEGER NOT NULL,
    link_type   INTEGER NOT NULL,
    tag         BLOB,

    FOREIGN KEY(action_hash) REFERENCES DhtAction(hash)
);

-- Deleted link index table.
--
-- For tracking which links have been deleted. Populated at integration time when DeleteLink ops are validated.
CREATE TABLE DeletedLink (
    action_hash      BLOB PRIMARY KEY,  -- The DeleteLink action
    create_link_hash BLOB NOT NULL,      -- The CreateLink being deleted

    FOREIGN KEY(action_hash) REFERENCES DhtAction(hash),
    FOREIGN KEY(create_link_hash) REFERENCES Link(action_hash)
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

/// Represents how entry data is included in an op.
pub enum OpEntry {
    /// The entry is present in this op
    Present(Entry),
    /// The action references a private entry, which is not included
    Hidden,
}

/// Chain operations that represent chain data distributed across the network.
/// Each operation is stored at a specific DHT location determined by its basis hash.
pub enum ChainOp {
    /// Stores the complete record at the record authority.
    ///
    /// OpEntry will be Present for public entries, Hidden for private entries.
    CreateRecord(SignedAction, OpEntry),
    /// Stores entry content at the entry authority.
    /// 
    /// Op type is only created for public entries.
    CreateEntry(SignedAction, Entry),
    /// Agent activity stored at the agent's authority.
    AgentActivity(SignedAction),
    /// Entry updates indexed at the original entry authority.
    ///
    /// Only created if the original entry was public.
    UpdateEntry(SignedAction, Entry),
    /// Updates indexed at the original record authority.
    UpdateRecord(SignedAction),
    /// Entry deletes indexed at the original entry authority.
    /// 
    /// Only created if the original entry was public.
    DeleteEntry(SignedAction),
    /// Deletes indexed at the original record authority.
    DeleteRecord(SignedAction),
    /// Links indexed at the base address.
    CreateLink(SignedAction),
    /// Link deletes indexed at the base address.
    DeleteLink(SignedAction),
}

/// Warrant operation representing a claim that a ChainOp was invalid.
pub struct WarrantOp(SignedWarrant);

/// Internal representation of a ChainOp after hash and signature verification.
///
/// This type is used internally after counterfeit checks have been completed. 
/// The hashes are verified once during the incoming ops workflow and retained.
pub(crate) struct HashedChainOp {
    /// The verified op hash
    pub op_hash: DhtOpHash,
    /// The signed action with verified hash
    pub action: SignedActionHashed,
    /// The entry with verified hash (if present in the op)
    pub entry: Option<EntryHashed>,
    /// The type of chain operation
    pub op_type: ChainOpType,
    /// The DHT location where this op is stored
    pub basis_hash: AnyDhtHash,
    /// The numeric storage center location (derived from basis_hash)
    pub storage_center_loc: u32,
}
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
   ├─> Insert into `CapGrant` or `CapClaim` table (if applicable)
   ├─> Insert into `Link` table (if action is CreateLink)
   └─> Insert into `DeletedLink` table (if action is DeleteLink)

3. Create ops for publishing
   ├─> If this is a countersigning action, skip this step (ops created on session completion)
   ├─> Transform action/entry into DHT ops (see "Action to Op Transform" below)
   └─> Insert into `AuthoredOp` with publishing state

4. Publish DHT Ops Workflow
   ├─> Query ops which are ready for publishing from `AuthoredOp` and `Warrant`
   ├─> Cross-database LEFT JOIN to `DhtOp` to check integration status
   ├─> Publish if: op not in `DhtOp` (outside arc) OR op in `DhtOp` with `validation_status = 'valid'`
   ├─> Group by `basis_hash` for efficient sending
   ├─> Send ops to DHT authorities over the network
   └─> Update `last_publish_time` in `AuthoredOp` (or `Warrant`)

5. Validation Receipt Workflow
   ├─> Receive validation receipts from validators
   ├─> Insert into `ValidationReceipt` table
   └─> Update `receipts_complete` in `AuthoredOp` when sufficient receipts received

6. Countersigning Workflow (if applicable)
   ├─> Lock the chain: Insert into `ChainLock` with session subject and expiration
   ├─> Wait for all participants to sign
   ├─> Verify all participants have signed
   ├─> Create ops from the countersigned action/entry
   ├─> Insert into `AuthoredOp` with publishing state
   ├─> Unlock the chain: Delete from `ChainLock` where author = current agent
   └─> Trigger publish workflow

   **Chain Lock Query:**
   ```sql
   -- Acquire lock
   INSERT INTO ChainLock (author, subject, expires_at_timestamp)
   VALUES (?, ?, ?)
   ON CONFLICT (author) DO UPDATE
   SET subject = excluded.subject,
       expires_at_timestamp = excluded.expires_at_timestamp;

   -- Check if chain is locked
   SELECT * FROM ChainLock
   WHERE author = ?
     AND expires_at_timestamp > CURRENT_TIMESTAMP;

   -- Release lock
   DELETE FROM ChainLock WHERE author = ?;

   -- Clean up expired locks
   DELETE FROM ChainLock WHERE expires_at_timestamp <= CURRENT_TIMESTAMP;
   ```
```

#### Action to Op Transform

When a new action (and optional entry) is authored, it must be transformed into one or more DHT ops for distribution. The specific ops created depend on the action type and whether the entry is private.

**Transform Rules:**

For **Create** actions:
- Always create `AgentActivity(SignedAction)` op (stored at agent's authority)
- Always create `CreateRecord(SignedAction, OpEntry)` op (stored at action hash authority)
  - For public entries: `OpEntry::Present(entry)`
  - For private entries: `OpEntry::Hidden`
- If action has a public entry: create `CreateEntry(SignedAction, Entry)` op (stored at entry hash authority)
- If action has a private entry: do NOT create `CreateEntry` op

For **Update** actions:
- Always create `AgentActivity(SignedAction)` op
- Always create `UpdateRecord(SignedAction)` op (stored at original action hash authority)
- If action has a public entry: create `UpdateEntry(SignedAction, Entry)` op (stored at original entry hash authority)
- If action has a private entry: do NOT create `UpdateEntry` op

For **Delete** actions:
- Always create `AgentActivity(SignedAction)` op
- Always create `DeleteRecord(SignedAction)` op (stored at original action hash authority)
- If the deleted action had a public entry: create `DeleteEntry(SignedAction)` op (stored at original entry hash authority)
- If the deleted action had a private entry: do NOT create `DeleteEntry` op

For **CreateLink** actions:
- Always create `AgentActivity(SignedAction)` op
- Always create `CreateLink(SignedAction)` op (stored at base address)

For **DeleteLink** actions:
- Always create `AgentActivity(SignedAction)` op
- Always create `DeleteLink(SignedAction)` op (stored at base address of the link being deleted)

For **Dna**, **AgentValidationPkg**, and **InitZomesComplete** actions:
- Always create `AgentActivity(SignedAction)` op
- No record or entry ops (these are chain-only actions)

**Private Entry Rationale:**

Private entries are handled differently in op creation:

1. **`CreateEntry`/`UpdateEntry` ops are never created for private entries**
   - These ops are stored at entry hash authorities and always contain the full entry
   - Creating them for private entries would expose the content to entry authorities
   - Private entries cannot be retrieved by entry hash, so entry authority storage serves no purpose

2. **`CreateRecord`/`UpdateRecord` ops use `OpEntry::Hidden` for private entries**
   - These ops are stored at action hash authorities chosen through DHT routing
   - `OpEntry::Hidden` indicates an entry exists but is not included in the op

3. **`DeleteEntry` ops are never created for private entries**
   - Since `CreateEntry` was never created, there's nothing at the entry authority to mark as deleted
   - `DeleteRecord` ops are sufficient to mark the action as deleted

**Note:**

The `private_entry` field in the Action table (or the `EntryVisibility` from entry type) is checked during op creation to determine:
- Whether to create `CreateEntry`/`UpdateEntry`/`DeleteEntry` ops (never for private)
- Whether to use `OpEntry::Hidden` or `OpEntry::Present` in `CreateRecord` ops

#### Publish Query and Op Construction

The publish workflow queries the authored database for ops that need publishing and constructs the full `ChainOp` (wrapped in `DhtOp`) for network transmission.

**Query Logic:**

The query selects from `AuthoredOp` table, joining to `Action` and optionally `Entry`, with a cross-database join to `DhtOp` to ensure only integrated ops are published. A UNION includes warrants from the `Warrant` table:

```sql
-- Chain ops
SELECT
    'chain' as op_category,
    Action.hash as action_hash,
    Action.author,
    Action.timestamp,
    Action.action_type,
    Action.action_data,
    Entry.blob as entry_blob,
    AuthoredOp.hash as op_hash,
    AuthoredOp.op_type,
    AuthoredOp.basis_hash,
    AuthoredOp.last_publish_time
FROM AuthoredOp
JOIN Action ON AuthoredOp.action_hash = Action.hash
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
-- Cross-database LEFT JOIN to check integration status
-- NULL means op is outside our arc (not stored locally, safe to publish)
-- Non-NULL means op is within our arc (must be integrated before publishing)
LEFT JOIN dht.DhtOp ON AuthoredOp.hash = dht.DhtOp.hash
WHERE
    Action.author = :author
    AND (AuthoredOp.last_publish_time IS NULL
        OR AuthoredOp.last_publish_time <= :recency_threshold)
    AND AuthoredOp.receipts_complete IS NULL
    AND (dht.DhtOp.hash IS NULL OR dht.DhtOp.validation_status = 'valid')

UNION ALL

-- Warrant ops
SELECT
    'warrant' as op_category,
    NULL as action_hash,
    Warrant.author,
    Warrant.timestamp,
    'Warrant' as action_type,
    NULL as action_data,
    NULL as entry_blob,
    Warrant.hash as op_hash,
    'Warrant' as op_type,
    Warrant.warrantee as basis_hash,  -- Warrants stored at warrantee's authority
    Warrant.last_publish_time
FROM Warrant
WHERE
    Warrant.author = :author
    AND (Warrant.last_publish_time IS NULL
        OR Warrant.last_publish_time <= :recency_threshold)
    AND Warrant.receipts_complete IS NULL

ORDER BY timestamp
```

**In-Memory Transform:**

For each row returned:
1. **Deserialize Action**: Reconstruct full `Action` from common fields + the `action_data` BLOB
2. **Construct ChainOp**: Build the appropriate `ChainOp` variant based on `op_type`:
   - `CreateRecord`: Requires `SignedAction` + `OpEntry`
     - If entry exists and is public: `OpEntry::Present(entry)`
     - If entry exists and is private: `OpEntry::Hidden`
     - If no entry: `OpEntry::NotStored` (for actions without entries)
   - `CreateEntry`: Requires `SignedAction` + `Entry` (entry must be present and public)
   - `AgentActivity`: Requires `SignedAction` + `Option<Entry>`
     - Entry is Some if `cached_at_agent_activity` is enabled for the entry type
   - `UpdateEntry`: Requires `SignedAction` + `Entry` (entry must be present and public)
   - `UpdateRecord`: Requires `SignedAction` only
   - `DeleteEntry`: Requires `SignedAction` only
   - `DeleteRecord`: Requires `SignedAction` only
   - `CreateLink`: Requires `SignedAction` only
   - `DeleteLink`: Requires `SignedAction` only
3. **Wrap in DhtOp**: Wrap the `ChainOp` in `DhtOp::ChainOp(Box::new(chain_op))`
4. **Group by Basis**: Collect ops by `basis_hash` for efficient network transmission

**Differences to Current Implementation:**

1. **Missing `withhold_publish` field**: The current code checks `DhtOp.withhold_publish IS NULL` to exclude countersigning ops. The new `AuthoredOp` schema doesn't include this field.
   - **Resolution**: Countersigning completion should produce ops from the action/entry instead of clearing a field. No `withhold_publish` field needed in `AuthoredOp` table. During the countersigning session, ops are not created at all - they are only created when the session successfully completes, at which point they are immediately publishable. This approach is superior because: (a) entries should not be served during active countersigning sessions since the session may fail, (b) it eliminates intermediate "withheld" state and associated cleanup complexity, (c) op existence semantically means "publishable data", and (d) failed sessions leave no garbage ops in the database.

2. **Missing `when_integrated` field**: The current code checks `DhtOp.when_integrated IS NOT NULL` to ensure ops are only published after local validation completes. The new `AuthoredOp` schema doesn't track integration.
   - **Resolution**: Use a read-only cross-database query instead of maintaining duplicate state. The publish query performs a LEFT JOIN with the DHT database's `DhtOp` table to check integration status: `LEFT JOIN dht.DhtOp ON AuthoredOp.hash = dht.DhtOp.hash WHERE dht.DhtOp.hash IS NULL OR dht.DhtOp.validation_status = 'valid'`. This approach is necessary because:
     - The agent's storage arc can change over time, making sequencing-based approaches fragile
     - Ops within the agent's arc must be integrated before publishing (so they can be served to peers)
     - Ops outside the agent's arc won't be in `DhtOp` at all, but still need to be published
   - The LEFT JOIN handles both cases: if the op is NULL in DhtOp (outside arc), publish immediately; if non-NULL (inside arc), only publish when validated. The cross-database join naturally handles arc changes by checking current state at publish time, avoiding stale state issues. While this requires using cross-database query features in `holochain_state` and `holochain_data`, it's the simplest approach that correctly handles all edge cases. No cross-database state updates are needed - integration status has a single source of truth in the `DhtOp` table.

3. **`op_order` field**: Not needed in the new design.
   - **Current Purpose**: `OpOrder` combines op type priority (0-9) with timestamp to ensure "the most likely ordering where dependencies will come first"
   - **New Approach**: Use `ORDER BY Action.seq, Action.timestamp` in the publish query
   - **Rationale**:
     - Sequence number naturally orders actions from earliest to latest in the chain
     - Publishing foundational data first helps nodes joining large networks validate more efficiently
     - Timestamp breaks ties when multiple ops reference the same action
     - Simpler than maintaining a computed ordering field
   - **Resolution**: No `op_order` column needed in `AuthoredOp`

### Validation Flow

The validation flow processes incoming DHT ops through several stages, from initial receipt through final integration into the DHT database.

```
1. Incoming DHT Ops Workflow
   ├─> Compute op hash (`DhtOpHash`) from op content
   ├─> Filter duplicate ops already being processed
   ├─> Verify counterfeit checks (signature and hash verification)
   ├─> Convert `ChainOp` to `HashedChainOp` (internal type with checked hashes)
   ├─> Filter ops already in database (check DhtOp table)
   ├─> Insert action into DhtAction with record_validity=NULL
   ├─> Insert entry (if applicable) into DhtEntry
   └─> Insert into LimboOp with validation_stage='pending_sys'

2. Sys Validation Workflow
   ├─> Query LimboOp for ops with validation_stage='pending_sys'
   ├─> Check dependencies in DhtAction/DhtEntry
   ├─> Perform sys validation checks
   └─> Update sys_validation_status in LimboOp
       └─> Trigger app validation if valid

3. App Validation Workflow (if sys valid)
   ├─> Query LimboOp for ops with validation_stage='pending_app'
   ├─> Run WASM validation callbacks
   └─> Update app_validation_status in LimboOp
       └─> Trigger integration if valid

4. Integration Workflow (if both valid)
   ├─> Query LimboOp for ops with validation_stage='complete'
   ├─> Move op from LimboOp to DhtOp (set when_integrated)
   ├─> Update DhtAction record_validity with aggregated status
   └─> Delete from LimboOp
```

#### Incoming DHT Ops Workflow

The incoming DHT ops workflow is the entry point for all DHT ops received from the network. It performs initial validation and inserts ops into limbo for further processing.

**Workflow Steps:**

1. **Compute Op Hash**
   - Use the `holo_hash` crate to compute the op hash from the full op content.

2. **Location check**
   - Verify that the location of the op hash falls within the current arc set for local agents.
   - If any ops don't fall within the expected arcs, reject the batch.

3. **Existence Check**
    - Query `DhtOp` table to filter ops already in database
   ```sql
   SELECT EXISTS(
       SELECT 1 FROM DhtOp WHERE DhtOp.hash = :hash
   )
   ```
    - Skip ops that already exist

4. **Deduplication Check**
   - Check the current working state to filter ops already being processed to prevent duplicate work for ops in flight.

5. **Counterfeit Checks and Hash Verification**
   - For each op, verify structural integrity and cryptographic validity:
     - **Signature verification**:
         - For `ChainOp`: `verify_action_signature(signature, action)` ensures signature is valid for the action
         - For `WarrantOp`: `verify_warrant_signature(warrant_op)` ensures warrant signature is valid
     - **Action hash verification**: Using `holo_hash` to compute the action hash ensures that the action content matches the provided action hash.
     - **Entry hash verification**: For actions with an entry, use `holo_hash` to verify entry content hashes to the entry hash referenced in the action.
   - **Convert to HashedChainOp**: After verification, convert each `ChainOp` to `HashedChainOp` containing the verified hashes. This avoids re-computing hashes during database insertion.
   - **Batch rejection**: If any op fails any check, the entire incoming batch is dropped
     - This prevents the invalid op from entering the system
     - It also prevents wasting resources processing other ops in the batch

6. **Insert Into Limbo**
   - Insert the batch of `HashedChainOp` into tables within a single write transaction
   - For each `HashedChainOp` in the batch:
     - Insert action from `action: SignedActionHashed` into `DhtAction` table with `record_validity = NULL`
     - Insert entry from `entry: Option<EntryHashed>` (if present) into `DhtEntry` table
     - Insert op metadata into `LimboOp` table using pre-computed hashes (`op_hash`, `action.hash`, `basis_hash`, `storage_center_loc`)
   - Set initial validation state in `LimboOp`:
     - `validation_stage = 'pending_sys'`
     - `sys_validation_status = NULL`
     - `app_validation_status = NULL`
     - `when_received = current_timestamp`
   - Set `require_receipt = true` to send validation receipts back to author

7. **Trigger Sys Validation**
   - Send trigger to `sys_validation_workflow` queue consumer
   - Workflow run completes

**Hash Verification Summary:**

All incoming ops undergo four critical hash verifications during counterfeit checks (step 4):
1. **Op hash**: Verified by `DhtOpHashed::from_content_sync()` - ensures op content matches provided hash
2. **Action hash**: Verified by `ActionHashed::from_content_sync()` - ensures action content matches action hash in op
3. **Entry hash**: Verified by `EntryHashed::from_content_sync()` - ensures entry content matches entry hash in action
4. **Signature**: Verified by `verify_action_signature()` or `verify_warrant_signature()` - ensures signature is valid

If any verification fails, the entire batch is rejected before database insertion.

After successful verification, each `ChainOp` is converted to `HashedChainOp` containing the verified hashes. This internal representation is used for database insertion (step 5), avoiding the need to re-compute hashes.

**Error Handling:**

- Invalid signature: Drop the entire batch
- Hash mismatch: Rejected during hash computation
- Wrong location: Drop the entire batch
- Database errors: Workflow returns an error, op batch is dropped

#### Sys Validation Workflow

The sys validation workflow performs system-level integrity checks on ops in the limbo table. It validates chain structure, action/entry consistency, and dependencies before allowing ops to proceed to app validation.

**Query for Ops Pending Sys Validation:**

```sql
-- Ops pending sys validation
SELECT
    LimboOp.hash,
    LimboOp.op_type,
    LimboOp.action_hash,
    LimboOp.sys_validation_attempts,
    DhtAction.author,
    DhtAction.entry_hash
FROM LimboOp
JOIN DhtAction ON LimboOp.action_hash = DhtAction.hash
WHERE
    LimboOp.validation_stage = 'pending_sys'
    AND LimboOp.sys_validation_status IS NULL
ORDER BY sys_validation_attempts, DhtAction.seq, when_received
LIMIT 10000
```

**Workflow Steps:**

1. **Query Pending Ops**
   - Select ops with `validation_stage = 'pending_sys'`
   - Order by sys validation attempts (least first), then action sequence, then received time
   - Limit to 10,000 ops per workflow run

2. **Fetch Dependencies**
   - Concurrently fetch dependencies from local databases
   - Store in `SysValDeps` validation dependency cache
   - Missing dependencies tracked for network fetch

3. **Run Sys Validation Checks**
   - For chain ops: validate chain structure, action/entry consistency
   - For warrant ops: validate warranted actions

4. **Update Validation Status**
   - For **valid** ops:
     ```sql
     UPDATE LimboOp
     SET validation_stage = 'pending_app',
         sys_validation_status = 'valid',
         last_validation_attempt = CURRENT_TIMESTAMP,
         sys_validation_attempts = sys_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For **rejected** ops:
     ```sql
     UPDATE LimboOp
     SET sys_validation_status = 'rejected',
         last_validation_attempt = CURRENT_TIMESTAMP,
         sys_validation_attempts = sys_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For ops with **missing dependencies**: no status update, retry after network fetch
     - TODO increment sys_validation_attempts

5. **Trigger Next Workflow**
   - If any ops accepted: trigger `app_validation_workflow`
   - If any warrant ops validated: trigger `integration_workflow`

6. **Fetch Missing Dependencies from Network**
   - For ops with missing dependencies, fetch actions from network
   - If dependencies fetched: re-trigger `sys_validation_workflow`
   - If still missing: sleep and retry later

#### App Validation Workflow

The app validation workflow executes application-defined validation logic via WASM for ops that have passed sys validation.

**Query for Ops Pending App Validation:**

```sql
SELECT
    LimboOp.hash,
    LimboOp.op_type,
    LimboOp.action_hash,
    LimboOp.app_validation_attempts,
    DhtAction.author,
    DhtAction.entry_hash
FROM LimboOp
JOIN DhtAction ON LimboOp.action_hash = DhtAction.hash
WHERE
    LimboOp.validation_stage = 'pending_app'
    AND LimboOp.sys_validation_status = 'valid'
    AND LimboOp.app_validation_status IS NULL
ORDER BY app_validation_attempts, DhtAction.seq, when_received
LIMIT 10000
```

**Workflow Steps:**

1. **Query Pending Ops**
   - Select ops with `validation_stage = 'pending_app'`
   - Order by app validation attempts (least first), then action sequence, then received time

2. **Execute WASM Validation**
   - Load appropriate DNA and ribosome
   - Call `validate(op: Op)` callback for the op
   - Collect validation outcome (`Valid`, `Rejected`, or dependency request)

3. **Update Validation Status**
   - For **valid** ops:
     ```sql
     UPDATE LimboOp
     SET validation_stage = 'complete',
         app_validation_status = 'valid',
         last_validation_attempt = CURRENT_TIMESTAMP,
         app_validation_attempts = app_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For **rejected** ops:
     ```sql
     UPDATE LimboOp
     SET app_validation_status = 'rejected',
         last_validation_attempt = CURRENT_TIMESTAMP,
         app_validation_attempts = app_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For ops **awaiting dependencies**:
     ```sql
     UPDATE LimboOp
     SET app_validation_attempts = app_validation_attempts + 1
     WHERE hash = :op_hash
     ```

4. **Trigger Integration**
   - If any ops valid: trigger `integration_workflow`

#### Integration Workflow

The integration workflow moves validated ops from the limbo table to the DHT table and updates the associated record validity status.

**Query for Ops Ready for Integration:**

```sql
SELECT
    LimboOp.hash,
    LimboOp.op_type,
    LimboOp.action_hash,
    LimboOp.basis_hash,
    LimboOp.sys_validation_status,
    LimboOp.app_validation_status
FROM LimboOp
WHERE
    LimboOp.validation_stage = 'complete'
    AND (LimboOp.sys_validation_status = 'valid' OR LimboOp.sys_validation_status = 'rejected')
    AND (LimboOp.app_validation_status = 'valid' OR LimboOp.app_validation_status = 'rejected')
ORDER BY LimboOp.when_received
```

**Workflow Steps:**

1. **Query Completed Ops**
   - Select ops with `validation_stage = 'complete'` and both sys and app validation statuses set.
   - Both valid and rejected ops are integrated.

2. **Move Op to DhtOp Table**
   - Start a write transaction for each op.
   - Execute the following insert:
   ```sql
   INSERT INTO DhtOp (
       hash,
       op_type,
       action_hash,
       basis_hash,
       storage_center_loc,
       validation_status,
       locally_validated,
       when_received,
       when_integrated
   )
   SELECT
       hash,
       op_type,
       action_hash,
       basis_hash,
       storage_center_loc,
       CASE
           WHEN sys_validation_status = 'valid' AND app_validation_status = 'valid' THEN 'valid'
           ELSE 'rejected'
       END,
       TRUE,  -- locally_validated
       when_received,
       CURRENT_TIMESTAMP
   FROM LimboOp
   WHERE hash = :op_hash
   ```

3. **Update `DhtAction` with Record Validity**
   - Aggregate `record_validity` from all ops for this action using a single query
   - **Rules (applied in SQL):**
     - If ANY op for this action is rejected: `record_validity = 'rejected'`
     - If at least one op is valid and none rejected: `record_validity = 'valid'`
   ```sql
   UPDATE DhtAction
   SET record_validity = (
       SELECT CASE
           WHEN COUNT(CASE WHEN validation_status = 'rejected' THEN 1 END) > 0 THEN 'rejected'
           WHEN COUNT(CASE WHEN validation_status = 'valid' THEN 1 END) > 0 THEN 'valid'
           ELSE NULL
       END
       FROM DhtOp
       WHERE action_hash = :action_hash
   )
   WHERE hash = :action_hash
   ```

   Note: DhtAction and DhtEntry rows are created during incoming op insertion (step 6) with `record_validity = NULL`

4. **Update Link Index Tables** (if applicable)
   - If the action is a `CreateLink` and the op is valid, insert into `Link` table:
   ```sql
   INSERT INTO Link (action_hash, base_hash, zome_index, link_type, tag)
   SELECT
       Action.hash,
       :base_hash,      -- extracted from action_data
       :zome_index,     -- extracted from action_data
       :link_type,      -- extracted from action_data
       :tag             -- extracted from action_data
   FROM DhtAction AS Action
   WHERE Action.hash = :action_hash
       AND Action.record_validity = 'valid'
       AND Action.action_type = 'CreateLink'
   ```
   - If the action is a `CreateLink` and the op is rejected, ensure no row exists in `Link` table for this action (delete if necessary).
   - If the action is a `DeleteLink` and the op is valid, insert into `DeletedLink` table:
   ```sql
   INSERT INTO DeletedLink (action_hash, create_link_hash)
   VALUES (:action_hash, :create_link_hash)  -- create_link_hash from action_data
   ```
   - If the action is a `DeleteLink` and the op is rejected, ensure no row exists in `DeletedLink` table for this action (delete if necessary).

5. **Delete from LimboOp**
   ```sql
   DELETE FROM LimboOp WHERE hash = :op_hash
   ```

6. **Commit Transaction**
    - Commit the write transaction to finalize changes for this op.

7. **Send Validation Receipt** (if required)
   - If op came from network and `require_receipt = true`
   - Send a signed validation receipt back to the author

### Record Validity Aggregation

**Rules:**
1. A record is **INVALID** if ANY known ops for it are rejected.
2. A record is **VALID** if at least one known op for it is valid and no known ops are rejected.
3. Records in limbo have a `NULL` value for `record_validity`. After integration of the first op for a record, `record_validity` is `'valid'` or `'rejected'`.

The record validity is determined at the time of integration by examining all known ops associated with the record. Due to the sharding model, a validator may not have all ops for a record - they hold specific op types over specific ranges. This aggregated status is stored with the record itself, eliminating the need for complex joins during queries.

### Record Validity Correction

TODO: It is a future piece of work to define this logic. When implemented, this logic must also clean up index tables (`Link`, `DeletedLink`, `CapGrant`, `CapClaim`) when records are invalidated.

## Query Patterns

### Get Record by Hash

Retrieve a complete record (action and entry) from the DHT by action hash.

```sql
SELECT Action.*, Entry.*
FROM DhtAction AS Action
LEFT JOIN DhtEntry AS Entry ON Action.entry_hash = Entry.hash
WHERE Action.hash = ?
  AND Action.record_validity = 'valid'
```

### Get Entry by Hash

Retrieve an entry from the DHT using the validation status from any action that references it.

```sql
SELECT Entry.*, Action.*
FROM DhtEntry AS Entry
JOIN DhtAction AS Action ON Action.entry_hash = Entry.hash
WHERE Entry.hash = ?
  AND Action.record_validity = 'valid'
LIMIT 1
```

Justification for *any*: An entry can be referenced by multiple actions. If at least one action referencing the entry is valid, the entry is considered valid to be served to the network or used.

### Get Links

Query all non-deleted links from a base hash using the Link index table populated at integration time.

```sql
-- Find non-deleted links from base
SELECT Link.*, Action.*
FROM Link
JOIN DhtAction AS Action ON Link.action_hash = Action.hash
LEFT JOIN DeletedLink ON Link.action_hash = DeletedLink.create_link_hash
WHERE Link.base_hash = ?
  AND DeletedLink.create_link_hash IS NULL  -- Exclude deleted links
  AND Action.record_validity = 'valid'
-- Additional filters can be applied in SQL or application code:
--   - link_type: WHERE Link.link_type = ?
--   - zome_index: WHERE Link.zome_index = ?
--   - tag prefix: WHERE Link.tag >= ? AND Link.tag < ?  (bytewise comparison)
--   - author: WHERE Action.author = ?
--   - timestamp bounds: WHERE Action.timestamp BETWEEN ? AND ?
```

### Count Links

Count non-deleted links matching query criteria using the `Link` index table.

```sql
-- Count non-deleted links
SELECT COUNT(*)
FROM Link
LEFT JOIN DeletedLink ON Link.action_hash = DeletedLink.create_link_hash
JOIN DhtAction AS Action ON Link.action_hash = Action.hash
WHERE Link.base_hash = ?
  AND DeletedLink.create_link_hash IS NULL
  AND Action.record_validity = 'valid'
-- Additional filters (link_type, tag prefix, author, timestamp) applied as needed
```

### Get Agent Activity

Query an agent's chain activity with flexible filtering for action types, entry types, and validation status.

```sql
SELECT Action.*
FROM DhtAction AS Action
WHERE Action.author = ?
  AND Action.record_validity IS NOT NULL
ORDER BY Action.seq
-- Application code filters for:
--   - Specific action_type values
--   - Entry types (via action_data deserialization)
--   - Sequence ranges
--   - Include valid/rejected/warrants based on GetActivityOptions
```

### Must Get Agent Activity

Deterministic hash-bounded query for agent activity used in countersigning scenarios.

```sql
-- Query with sequence bounds
SELECT Action.*
FROM DhtAction AS Action
WHERE Action.author = ?
  AND Action.seq BETWEEN ? AND ?  -- or bounded by prev_hash chain traversal
  AND Action.record_validity IS NOT NULL
ORDER BY Action.seq
-- Application code verifies that there are no gaps in sequence numbers (complete chain segment)
```

### Cap Grant and Claim Lookups

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

### Cache Pruning Strategy

TODO: Design how to track and prune cached ops when storage pressure occurs.

## Warrant Handling

Warrants require special consideration:

1. **ChainIntegrityWarrant**: Proves an author broke chain rules
   - Stays in `LimboOp` until the warranted action is fetched and validated
   - If warranted action is rejected, warrant moves to `DhtOp` as valid
   - If warranted action is valid, the warrant is rejected and removed

2. **ChainForkWarrant**: Proves an author has forked their chain
   - Stays in `LimboOp` until both forked actions are fetched and checked
   - If both forked actions are at the same sequence number, warrant moves to `DhtOp` as valid
   - If the forked actions do not match the fork condition, the warrant is rejected and removed

## Data Integrity Invariants

The system maintains these invariants:

1. **No op exists in both `LimboOp` and `DhtOp` simultaneously**
2. **Every DhtOp has a definite `validation_status` (never `NULL`)**
3. **`DhtAction` has a `NULL` value for `record_validity` for pending records, 'valid' or 'rejected' for integrated records**
4. **Ops are moved from limbo to DHT atomically with `record_validity` updates**
5. **Rejected ops in `DhtOp` cause their records to be marked 'rejected'**
6. **Dependencies are resolved before validation proceeds**
7. **Queries for validated data always check `record_validity IS NOT NULL` or `record_validity = 'valid'`**
