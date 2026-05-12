# Data State Model

## Overview

The Holochain data storage and validation architecture provides:

1. A single per-DNA database storing both agents' authored [action and entry](./data_model.md) chains and DHT data.
2. Ops as the unit of validation, with an aggregated validity status for [records](./data_model.md). 
3. Validation limbo tables (`LimboChainOp`, `LimboWarrant`) to track pending ops from the network, with shared `Action`/`Entry` tables.
4. Unified data querying across authored, obligated, and cached data in a single database.

## Architecture

### Core Principles

1. **Ops are the unit of validation**: All validation happens at the op level.
2. **Records aggregate op validity**: A record's validity is derived from the ops produced from it.
3. **Validation limbo isolates pending ops**: Unvalidated ops from the network stay in `LimboChainOp` (warrants in `LimboWarrant`) until validated, with actions and entries in shared `Action`/`Entry` tables marked by NULL `record_validity`. Self-authored ops bypass limbo (pre-validated at authoring time) but still go through integration to register in the DHT model.
4. **Single database per DNA**: Each DNA cell uses one database for all chain, DHT, and validation data. Private entries are isolated in a dedicated table for access control auditing.
5. **Unified data storage**: The database serves authored chain data, obligated DHT data, and cached data. Cached data can be distinguished by arc coverage as required.
6. **Clear state transitions**: Data moves through well-defined states with no ambiguity.
7. **Action-type specific handling**: Different action types (Create, Update, Delete, Link) have specific handling rules for constructing a DHT state that is queryable.

### Database Structure

#### Per-DNA Database

Each DNA cell uses a single database for authored chain data, DHT data, and validation state.

```sql
-- Actions (stores both locally authored and network-received actions).
--
-- WITHOUT ROWID stores rows in a single B-tree clustered on hash, eliminating the
-- double-lookup overhead of a regular table (rowid B-tree + primary key index).
-- All FK references already store the full 32-byte hash, so no downstream changes needed.
CREATE TABLE Action (
   hash          BLOB PRIMARY KEY,
   author        BLOB NOT NULL,
   seq           INTEGER NOT NULL,
   prev_hash     BLOB,         -- NULL for the genesis action (seq = 0) only
   timestamp     INTEGER NOT NULL,
   action_type   INTEGER NOT NULL, -- ActionType enum variant
   action_data   BLOB NOT NULL,    -- Serialized ActionData enum
   signature     BLOB NOT NULL,    -- Author's signature over the action

   -- Reference fields for entry meta
   entry_hash    BLOB,         -- NULL for non-entry actions
   private_entry BOOLEAN,      -- Cached visibility flag for the referenced entry; NULL for non-entry actions

   -- Record validity (aggregated from all ops for this record)
   -- A record is the combination of action + entry (if applicable)
   -- Self-authored records are inserted with record_validity = 1 (pre-validated)
   -- Network-received records are inserted with record_validity = NULL (pending validation)
   record_validity INTEGER -- NULL=pending, 1=accepted, 2=rejected
) WITHOUT ROWID;

-- Public entries.
--
-- WITHOUT ROWID for the same reason as Action: single clustered B-tree on hash,
-- accessed almost exclusively by primary key.
CREATE TABLE Entry (
   hash BLOB PRIMARY KEY,
   blob BLOB NOT NULL
) WITHOUT ROWID;

-- Private entries (author's own private entries only).
--
-- Kept in a dedicated table separate from Entry so that access to private entry
-- content is isolated and auditable. Private entries are never distributed to other
-- agents, so only an author's private entries appear here.
CREATE TABLE PrivateEntry (
   hash   BLOB PRIMARY KEY,
   author BLOB NOT NULL,    -- The agent who authored this entry
   blob   BLOB NOT NULL
) WITHOUT ROWID;

-- Scheduled function records (per-author within this DNA's DB).
--
-- Records scheduled zome-function invocations originated by an author within
-- this DNA. Per-author state, but lives in the per-DNA DB; the row's `author`
-- column distinguishes rows for different agents on the same DNA.
CREATE TABLE ScheduledFunction (
   author         BLOB    NOT NULL,
   zome_name      TEXT    NOT NULL,
   scheduled_fn   TEXT    NOT NULL,
   maybe_schedule BLOB    NOT NULL,      -- Serialized Option<Schedule>
   start_at       INTEGER NOT NULL,      -- Microsecond timestamp the function becomes live
   end_at         INTEGER NOT NULL,      -- Microsecond timestamp the function expires
   ephemeral      INTEGER NOT NULL,      -- 0/1 — 1 means the row is removed once it fires

   PRIMARY KEY (author, zome_name, scheduled_fn)
) WITHOUT ROWID;

-- Capability grants lookup table.
--
-- For simpler querying of cap grants from the agent chain.
CREATE TABLE CapGrant (
    action_hash BLOB PRIMARY KEY,
    cap_access  INTEGER NOT NULL, -- CapAccess enum: 0=unrestricted, 1=transferable, 2=assigned
    tag         TEXT,

    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- Capability claims table.
--
-- For recording capability grants received from other agents. Cap claims are not
-- chain entries; they are stored locally as an index for exercising received capabilities.
CREATE TABLE CapClaim (
    id      INTEGER PRIMARY KEY, -- rowid alias; secrets are credentials not identifiers
    author  BLOB NOT NULL,       -- The agent this claim belongs to
    tag     TEXT NOT NULL,
    grantor BLOB NOT NULL,
    secret  BLOB NOT NULL
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

-- Limbo for chain ops received from the network which are in the process of being validated.
-- Self-authored ops bypass limbo and are inserted directly into ChainOp,
-- but still go through integration to populate index tables.
CREATE TABLE LimboChainOp (
    hash        BLOB PRIMARY KEY,
    op_type     INTEGER NOT NULL, -- ChainOpType enum variant
    action_hash BLOB NOT NULL,

    -- DHT location
    basis_hash         BLOB NOT NULL,
    storage_center_loc INTEGER NOT NULL,

    -- Local validation state
    sys_validation_status INTEGER, -- NULL=pending, 1=accepted, 2=rejected
    app_validation_status INTEGER, -- NULL=pending, 1=accepted, 2=rejected
    abandoned_at          INTEGER,       -- NULL unless validation was abandoned due to unresolvable dependencies

    -- Validation receipt requirement
    require_receipt BOOLEAN NOT NULL,    -- Whether to send validation receipt back to author

    -- Timing and attempt tracking
    when_received INTEGER NOT NULL,
    sys_validation_attempts INTEGER DEFAULT 0,
    app_validation_attempts INTEGER DEFAULT 0,
    last_validation_attempt INTEGER,

    -- Storage tracking
    serialized_size INTEGER NOT NULL,  -- Size in bytes, calculated when op arrives

    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- Limbo for DHT warrant ops which are in the process of being validated.
-- Warrants have sys validation only; there is no app validation step.
CREATE TABLE LimboWarrant (
    hash      BLOB PRIMARY KEY,
    author    BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    warrantee BLOB NOT NULL,
    proof     BLOB NOT NULL,  -- Serialized WarrantProof (InvalidChainOp or ChainFork)

    -- DHT location (stored at warrantee's agent authority)
    storage_center_loc INTEGER NOT NULL,

    -- Local validation state
    sys_validation_status INTEGER, -- NULL=pending, 1=accepted, 2=rejected
    abandoned_at          INTEGER, -- NULL unless validation was abandoned due to unresolvable dependencies

    -- Timing and attempt tracking
    when_received           INTEGER NOT NULL,
    sys_validation_attempts INTEGER DEFAULT 0,
    last_validation_attempt INTEGER,

    -- Storage tracking
    serialized_size INTEGER NOT NULL
);

-- Integrated DHT chain ops.
--
-- Contains both self-authored ops (inserted directly at authoring time) and
-- network-received ops (moved from LimboChainOp after validation).
CREATE TABLE ChainOp (
    hash        BLOB PRIMARY KEY,
    op_type     INTEGER NOT NULL, -- ChainOpType enum variant
    action_hash BLOB NOT NULL,

    -- DHT location
    basis_hash         BLOB NOT NULL,
    storage_center_loc INTEGER NOT NULL,

    -- Final validation result
    validation_status INTEGER NOT NULL, -- 1=accepted, 2=rejected
    locally_validated BOOLEAN NOT NULL, -- whether this op was validated by us, or fetched from an authority

    -- Timing
    when_received   INTEGER NOT NULL, -- set at authoring time for self-authored ops, or copied from LimboChainOp
    when_integrated INTEGER NOT NULL, -- set at authoring time for self-authored ops, or when moved out of LimboChainOp

    -- Storage tracking
    serialized_size INTEGER NOT NULL, -- size in bytes for storage quota management

    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- Publishing state for locally authored chain ops.
--
-- Only rows for self-authored ops appear here; network-received ops have no publish state.
-- Separated from ChainOp to avoid sparse NULL columns in the much larger set of
-- network-received ops.
CREATE TABLE ChainOpPublish (
    op_hash           BLOB PRIMARY KEY,
    last_publish_time INTEGER,
    receipts_complete BOOLEAN,

    FOREIGN KEY(op_hash) REFERENCES ChainOp(hash)
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

    FOREIGN KEY(op_hash) REFERENCES ChainOp(hash)
);

-- Integrated DHT warrants.
--
-- Contains both self-authored warrants (inserted directly at authoring time) and
-- network-received warrants (moved from LimboWarrant after validation).
CREATE TABLE Warrant (
    hash      BLOB PRIMARY KEY,
    author    BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    warrantee BLOB NOT NULL,
    proof     BLOB NOT NULL,  -- Serialized WarrantProof (InvalidChainOp or ChainFork)

    -- DHT location (stored at warrantee's agent authority)
    storage_center_loc INTEGER NOT NULL
);

-- Publishing state for locally authored warrants.
--
-- Only rows for self-authored warrants appear here; network-received warrants have no publish state.
CREATE TABLE WarrantPublish (
    warrant_hash      BLOB PRIMARY KEY,
    last_publish_time INTEGER,

    FOREIGN KEY(warrant_hash) REFERENCES Warrant(hash)
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

    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
);

-- Deleted link index table.
--
-- For tracking which links have been deleted. Populated at integration time when DeleteLink ops are validated.
CREATE TABLE DeletedLink (
    action_hash      BLOB PRIMARY KEY,  -- The DeleteLink action
    create_link_hash BLOB NOT NULL,      -- The CreateLink being deleted

    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
);

-- Update index table.
--
-- For efficient update queries. Populated at integration time when Update actions are validated.
CREATE TABLE UpdatedRecord (
    action_hash              BLOB PRIMARY KEY,  -- The Update action
    original_action_hash     BLOB NOT NULL,      -- The original Create or Update being updated
    original_entry_hash      BLOB NOT NULL,      -- The original entry being updated

    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
);

-- Delete index table.
--
-- For efficient delete queries. Populated at integration time when Delete actions are validated.
CREATE TABLE DeletedRecord (
    action_hash         BLOB PRIMARY KEY,  -- The Delete action
    deletes_action_hash BLOB NOT NULL,      -- The action being deleted
    deletes_entry_hash  BLOB NOT NULL,      -- The entry being deleted

    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
);

```

**Differences to Current Implementation (Database Structure):**

1. **Merged authored and DHT databases**: The current implementation uses separate per-agent authored and per-DNA DHT databases. The new design uses a single per-DNA database. For full-arc nodes (the common case), every authored write already results in a DHT write, so the separate write handle provides no meaningful performance benefit. The single database eliminates cross-database query complexity.

2. **Private entries in dedicated table**: Private entries move from the general `Entry` table to a dedicated `PrivateEntry` table with an `author` column. This makes access control auditable at the schema level — code that queries `Entry` cannot accidentally leak private content.

3. **Self-authored ops bypass limbo but still integrate**: Self-authored ops are pre-validated at authoring time and inserted directly into `ChainOp` with `validation_status = 1`, bypassing limbo. However, they still go through the integration step (populating index tables like `Link`, `DeletedLink`, `UpdatedRecord`, `DeletedRecord`) so the DHT model reflects the authored data. The existing duplicate check in the incoming ops workflow (`SELECT EXISTS(SELECT 1 FROM ChainOp WHERE hash = :hash)`) prevents double-processing when the same op arrives from the network.

4. **Publishing state in dedicated tables**: Publishing fields (`last_publish_time`, `receipts_complete`) move from the removed `AuthoredChainOp` table into dedicated `ChainOpPublish` and `WarrantPublish` tables. Only locally authored ops/warrants have rows in these tables, avoiding sparse NULL columns in the much larger set of network-received data.

5. **`CapClaim` gains `author` column**: Since the table is no longer in a per-agent database, an `author` column distinguishes which agent each claim belongs to. All cap claim queries filter by author.

6. **`ValidationReceipt` references `ChainOp`**: Receipts now reference the integrated ops table directly instead of the removed `AuthoredChainOp` table.

7. **Removed tables**: `AuthoredChainOp`, `AuthoredWarrantOp`, and the separate authored `Action`/`Entry` tables are removed. Their data is consolidated into the DHT tables.

### Rust Structure

Actions:

```rust
/// Common action header stored for all action types
pub struct ActionHeader {
    pub author: AgentPubKey,
    pub timestamp: Timestamp,
    pub action_seq: u32,
    pub prev_action: Option<ActionHash>,
}

/// Action-specific data stored separately from header.
///
/// The `action_type` INTEGER column stores the discriminant:
///   1=Dna, 2=AgentValidationPkg, 3=InitZomesComplete, 4=Create,
///   5=Update, 6=Delete, 7=CreateLink, 8=DeleteLink, 9=CloseChain, 10=OpenChain
pub enum ActionData {
    Dna(DnaData),
    AgentValidationPkg(AgentValidationPkgData),
    InitZomesComplete(InitZomesCompleteData),
    Create(CreateData),
    Update(UpdateData),
    Delete(DeleteData),
    CreateLink(CreateLinkData),
    DeleteLink(DeleteLinkData),
    CloseChain(CloseChainData),
    OpenChain(OpenChainData),
}

/// Full action
pub struct Action {
    pub hash: ActionHash,
    pub header: ActionHeader,
    pub data: ActionData,
}

// Action-specific data structures (without redundant common fields)
pub struct CreateData {
    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

pub struct UpdateData {
    pub original_action_address: ActionHash,
    pub original_entry_address: EntryHash,
    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

pub struct DeleteData {
    pub deletes_address: ActionHash,
    pub deletes_entry_address: EntryHash,
}

pub struct CreateLinkData {
    pub base_address: AnyLinkableHash,
    pub target_address: AnyLinkableHash,
    pub zome_index: ZomeIndex,
    pub link_type: LinkType,
    pub tag: LinkTag,
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

pub struct CloseChainData {
    pub new_target: Option<MigrationTarget>,
}

pub struct OpenChainData {
    pub prev_target: MigrationTarget,
    /// Hash of the matching `CloseChain` action on the old chain.
    pub close_hash: ActionHash,
}
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
    /// The action type doesn't have an associated entry
    ActionOnly,
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
    CreateEntry(SignedAction, OpEntry),
    /// Agent activity stored at the agent's authority.
    AgentActivity(SignedAction),
    /// Entry updates indexed at the original entry authority.
    ///
    /// Only created if the original entry was public.
    UpdateEntry(SignedAction, OpEntry),
    /// Updates indexed at the original record authority.
    UpdateRecord(SignedAction, OpEntry),
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
   ├─> Insert into `Action` table with `record_validity = 1` (pre-validated)
   ├─> Insert into `Entry` (public) or `PrivateEntry` (private) table (if applicable)
   └─> Insert into `CapGrant` table (if action is a CapGrant entry)

3. Create ops for publishing
   ├─> If this is a countersigning action, skip this step (ops created on session completion)
   ├─> Transform action/entry into DHT ops (see "Action to Op Transform" below)
   └─> Insert directly into `ChainOp` with `validation_status = 1`, `locally_validated = TRUE`
   └─> Insert into `ChainOpPublish` with initial publishing state

4. Publish DHT Ops Workflow
   ├─> Query `ChainOpPublish` and `WarrantPublish` joined to their parent tables for ops that need publishing
   ├─> Only publish ops with `validation_status = 1` (accepted)
   ├─> Group by `basis_hash` for efficient sending
   ├─> Send ops to DHT authorities over the network
   └─> Update `last_publish_time` in `ChainOpPublish` (or `WarrantPublish`)

5. Validation Receipt Workflow
   ├─> Receive validation receipts from validators
   ├─> Insert into `ValidationReceipt` table
   └─> Update `receipts_complete` in `ChainOpPublish` when sufficient receipts received

6. Countersigning Workflow (if applicable)
   ├─> Lock the chain: Insert into `ChainLock` with session subject and expiration
   ├─> Wait for all participants to sign
   ├─> Verify all participants have signed
   ├─> Create ops from the countersigned action/entry
   ├─> Insert directly into `ChainOp` with `validation_status = 1`
   ├─> Insert into `ChainOpPublish` with initial publishing state
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
     AND expires_at_timestamp > unixepoch();

   -- Release lock
   DELETE FROM ChainLock WHERE author = ?;

   -- Clean up expired locks
   DELETE FROM ChainLock WHERE expires_at_timestamp <= unixepoch();
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
- Always create `UpdateRecord(SignedAction, OpEntry)` op (stored at original action hash authority)
  - For public entries: `OpEntry::Present(entry)`
  - For private entries: `OpEntry::Hidden`
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

The `private_entry` column on the `Action` table caches the entry's visibility at write time so op creation does not need to deserialize `action_data` or look up the entry type. It is checked during op creation to determine:
- Whether to create `CreateEntry`/`UpdateEntry`/`DeleteEntry` ops (never for private)
- Whether to use `OpEntry::Hidden` or `OpEntry::Present` in `CreateRecord` ops

#### Publish Query and Op Construction

The publish workflow queries `ChainOpPublish` and `WarrantPublish` (joined to their parent tables) for ops authored by the local agent that need publishing, and constructs the full `ChainOp` (wrapped in `DhtOp`) for network transmission.

**Query Logic:**

The query selects from `ChainOpPublish` joined to `ChainOp`, `Action`, and optionally `Entry`, filtering to the local agent's accepted ops. A UNION includes warrants from `WarrantPublish` joined to `Warrant`:

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
    ChainOp.hash as op_hash,
    ChainOp.op_type,
    ChainOp.basis_hash,
    pub.last_publish_time
FROM ChainOpPublish pub
JOIN ChainOp ON pub.op_hash = ChainOp.hash
JOIN Action ON ChainOp.action_hash = Action.hash
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE
    Action.author = :author
    AND ChainOp.validation_status = 1
    AND (pub.last_publish_time IS NULL
        OR pub.last_publish_time <= :recency_threshold)
    AND pub.receipts_complete IS NULL

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
    pub.last_publish_time
FROM WarrantPublish pub
JOIN Warrant ON pub.warrant_hash = Warrant.hash
WHERE
    Warrant.author = :author
    AND (pub.last_publish_time IS NULL
        OR pub.last_publish_time <= :recency_threshold)

ORDER BY timestamp, op_type
```

**In-Memory Transform:**

For each row returned:
1. **Deserialize `Action`**: Reconstruct full `Action` from common fields + the `action_data` BLOB
2. **Construct `ChainOp`**: Build the appropriate `ChainOp` variant based on `op_type`:
   - `CreateRecord`: Requires `SignedAction` + `OpEntry`
     - If entry exists and is public: `OpEntry::Present(entry)`
     - If entry exists and is private: `OpEntry::Hidden`
     - If no entry: `OpEntry::ActionOnly` (for actions without entries)
   - `CreateEntry`: Requires `SignedAction` + `Entry` (entry must be present and public)
   - `AgentActivity`: Requires `SignedAction`
   - `UpdateEntry`: Requires `SignedAction` + `Entry` (entry must be present and public)
   - `UpdateRecord`: Requires `SignedAction` and the new `OpEntry`
   - `DeleteEntry`: Requires `SignedAction` only
   - `DeleteRecord`: Requires `SignedAction` only
   - `CreateLink`: Requires `SignedAction` only
   - `DeleteLink`: Requires `SignedAction` only
3. **Wrap in `DhtOp`**: Wrap the `ChainOp` in `DhtOp::ChainOp(Box::new(chain_op))`
4. **Group by `basis_hash`**: Collect ops by `basis_hash` for efficient network transmission

**Differences to Current Implementation:**

1. **Merged database eliminates cross-database publish query**: The current implementation uses separate authored and DHT databases, requiring a cross-database LEFT JOIN to check integration status before publishing. With the single database, self-authored ops are inserted directly into `ChainOp` with `validation_status = 1` at authoring time. The publish query simply filters by `validation_status = 1` within the same database.

2. **No `withhold_publish` field**: The current code checks `ChainOp.withhold_publish IS NULL` to exclude countersigning ops. Countersigning completion produces ops from the action/entry instead of clearing a field. During the countersigning session, ops are not created at all — they are only created when the session successfully completes, at which point they are immediately publishable.

3. **No `op_order` field**: The current `OpOrder` combines op type priority with timestamp. The publish query uses `ORDER BY timestamp, op_type` instead, providing consistent chronological ordering across chain ops and warrants with a stable tiebreak.

### Validation Flow

The validation flow processes incoming DHT ops through several stages, from initial receipt through final integration into the DHT database.

```
1. Incoming DHT Ops Workflow (network-received ops only; self-authored ops bypass limbo)
   ├─> Compute op hash (`DhtOpHash`) from op content
   ├─> Filter duplicate ops already being processed
   ├─> Verify counterfeit checks (signature and hash verification)
   ├─> Convert `ChainOp` to `HashedChainOp` (internal type with checked hashes)
   ├─> Filter ops already in database (check `ChainOp` or `Warrant` table)
   ├─> Insert action into `Action` with `record_validity=NULL`
   ├─> Insert entry (if applicable) into `Entry`
   └─> Insert into `LimboChainOp` with `sys_validation_status=NULL`

2. Sys Validation Workflow
   ├─> Query `LimboChainOp` for ops with `sys_validation_status IS NULL`
   ├─> Check dependencies in `Action`/`Entry`
   ├─> Perform sys validation checks
   └─> Update `sys_validation_status` in `LimboChainOp`
       └─> Trigger app validation if accepted
       └─> Issue a warrant, and trigger integration if rejected

3. App Validation Workflow (if sys accepted)
   ├─> Query `LimboChainOp` for ops with `sys_validation_status=0 AND app_validation_status IS NULL`
   ├─> Run WASM validation callbacks
   └─> Update `app_validation_status` in `LimboChainOp`
       └─> Trigger integration if accepted
       └─> Issue a warrant, and trigger integration if rejected

4. Integration Workflow (if both accepted)
   ├─> Query `LimboChainOp` for ops where sys or app validation reached a terminal state
   ├─> Move op from `LimboChainOp` to `ChainOp` (set `when_integrated`)
   ├─> Update `Action` `record_validity` with aggregated status
   └─> Delete from `LimboChainOp`
```

#### Incoming DHT Ops Workflow

The incoming DHT ops workflow is the entry point for all DHT ops received from the network. It performs initial validation and inserts ops into limbo for further processing.

**Workflow Steps:**

1. **Compute and Verify Op Hash**
   - Use the `holo_hash` crate to compute the op hash from the full op content and verify it matches the hash provided with the op.

2. **Location check**
   - Verify that the location of the op hash falls within the current arc set for local agents.
   - If any ops don't fall within the expected arcs, reject the batch.

3. **Existence Check**
   - Skip ops that already exist in the integrated tables:
   ```sql
   -- For chain ops:
   SELECT EXISTS(SELECT 1 FROM ChainOp WHERE hash = :hash)
   -- For warrant ops:
   SELECT EXISTS(SELECT 1 FROM Warrant WHERE hash = :hash)
   ```

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
     - This prevents the rejected op from entering the system
     - It also prevents wasting resources processing other ops in the batch

6. **Expand to other op types**
    - For each record in the incoming batch, run the expansion to ops.
    - Any ops that weren't part of the incoming set, but do fall within our storage arc, should be added to the set.
    - This may save work fetching ops that we can generate locally, but the primary reason is to make it more difficult
      for peers to withhold individual op types when publishing a record.

7. **Insert Into Limbo**
   - Insert the batch of ops into tables within a single write transaction
   - For each op in the batch:
     - **If chain op (`DhtOp::ChainOp`)**:
       - Insert action from `action: SignedActionHashed` into `Action` table with `record_validity = NULL`, populating `signature` from the `SignedActionHashed` and `private_entry` from the action's entry visibility (NULL for non-entry actions)
       - Insert entry from `entry: Option<EntryHashed>` (if present) into `Entry` table
       - Insert op metadata into `LimboChainOp` using pre-computed hashes (`op_hash`, `action.hash`, `basis_hash`, `storage_center_loc`)
       - Set initial state: `sys_validation_status = NULL`, `app_validation_status = NULL`, `when_received = current_timestamp`, `serialized_size = encoded size`, `require_receipt = true`
     - **If warrant op (`DhtOp::WarrantOp`)**:
       - No action or entry insertion (warrants don't have actions)
       - Insert into `LimboWarrant` with the full warrant content (`hash`, `author`, `timestamp`, `warrantee`, `proof`, `storage_center_loc`)
       - Set initial state: `sys_validation_status = NULL`, `when_received = current_timestamp`, `serialized_size = encoded size`

8. **Trigger Sys Validation**
   - Send trigger to `sys_validation_workflow` queue consumer
   - Workflow run completes

**Hash Verification Summary:**

All incoming ops undergo four critical hash verifications during counterfeit checks (step 5):
1. **Op hash**: Verified by `DhtOpHashed::from_content_sync()` - ensures op content matches provided hash
2. **Action hash**: Verified by `ActionHashed::from_content_sync()` - ensures action content matches action hash in op
3. **Entry hash**: Verified by `EntryHashed::from_content_sync()` - ensures entry content matches entry hash in action
4. **Signature**: Verified by `verify_action_signature()` or `verify_warrant_signature()` - ensures signature is valid

If any verification fails, the entire batch is rejected before database insertion.

After successful verification, each `ChainOp` is converted to `HashedChainOp` containing the verified hashes. This internal representation is used for database insertion (step 7), avoiding the need to re-compute hashes.

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
    LimboChainOp.hash,
    LimboChainOp.op_type,
    LimboChainOp.action_hash,
    LimboChainOp.sys_validation_attempts,
    Action.author,
    Action.entry_hash
FROM LimboChainOp
JOIN Action ON LimboChainOp.action_hash = Action.hash
WHERE
    LimboChainOp.sys_validation_status IS NULL
ORDER BY sys_validation_attempts, Action.seq, when_received
LIMIT 10000
```

**Workflow Steps:**

1. **Query Pending Ops**
   - Select ops with `sys_validation_status IS NULL`
   - Order by sys validation attempts (least first), then action sequence, then received time
   - Limit to 10,000 ops per workflow run

2. **Fetch Dependencies**
   - Concurrently fetch dependencies from local databases
   - Store in `SysValDeps` validation dependency cache
   - Missing dependencies tracked for network fetch

3. **Run Sys Validation Checks**
   - Validate chain structure and action/entry consistency
   - Warrant sys validation is handled by the separate warrant validation workflow (see Warrant Handling)

4. **Update Validation Status**
   - For **accepted** ops:
     ```sql
     UPDATE LimboChainOp
     SET sys_validation_status = 1,
         last_validation_attempt = unixepoch(),
         sys_validation_attempts = sys_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For **rejected** ops:
     ```sql
     UPDATE LimboChainOp
     SET sys_validation_status = 2,
         last_validation_attempt = unixepoch(),
         sys_validation_attempts = sys_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For ops with **missing dependencies**: no status change, but increment attempt counter so retry ordering and runaway detection work correctly
     ```sql
     UPDATE LimboChainOp
     SET last_validation_attempt = unixepoch(),
         sys_validation_attempts = sys_validation_attempts + 1
     WHERE hash = :op_hash
     ```

5. **Trigger Next Workflow**
   - If any ops accepted: trigger `app_validation_workflow`

6. **Fetch Missing Dependencies from Network**
   - For ops with missing dependencies, fetch actions from network
   - If dependencies fetched: re-trigger `sys_validation_workflow`
   - If still missing: sleep and retry later

#### App Validation Workflow

The app validation workflow executes application-defined validation logic via WASM for ops that have passed sys validation.

**Query for Ops Pending App Validation:**

```sql
SELECT
    LimboChainOp.hash,
    LimboChainOp.op_type,
    LimboChainOp.action_hash,
    LimboChainOp.app_validation_attempts,
    Action.author,
    Action.entry_hash
FROM LimboChainOp
JOIN Action ON LimboChainOp.action_hash = Action.hash
WHERE
    LimboChainOp.sys_validation_status = 1
    AND LimboChainOp.app_validation_status IS NULL
ORDER BY app_validation_attempts, Action.seq, when_received
LIMIT 10000
```

**Workflow Steps:**

1. **Query Pending Ops**
   - Select ops with `sys_validation_status = 1 AND app_validation_status IS NULL`
   - Order by app validation attempts (least first), then action sequence, then received time
   - Limit to 10,000 ops per workflow run

2. **Execute WASM Validation**
   - Load appropriate DNA and ribosome
   - Call `validate(op: DhtOp)` callback for the op
   - Collect validation outcome (`Accepted`, `Rejected`, or dependency request)

3. **Update Validation Status**
   - For **accepted** ops:
     ```sql
     UPDATE LimboChainOp
     SET app_validation_status = 1,
         last_validation_attempt = unixepoch(),
         app_validation_attempts = app_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For **rejected** ops:
     ```sql
     UPDATE LimboChainOp
     SET app_validation_status = 2,
         last_validation_attempt = unixepoch(),
         app_validation_attempts = app_validation_attempts + 1
     WHERE hash = :op_hash
     ```
   - For ops **awaiting dependencies**: no status change, but increment attempt counter and update last attempt time so retry ordering and runaway detection work correctly
     ```sql
     UPDATE LimboChainOp
     SET last_validation_attempt = unixepoch(),
         app_validation_attempts = app_validation_attempts + 1
     WHERE hash = :op_hash
     ```

4. **Trigger Integration**
   - If any ops accepted: trigger `integration_workflow`

#### Integration Workflow

The integration workflow moves validated ops from the limbo table to the DHT table and updates the associated record validity status.

**Query for Ops Ready for Integration:**

```sql
SELECT
    LimboChainOp.hash,
    LimboChainOp.op_type,
    LimboChainOp.action_hash,
    LimboChainOp.basis_hash,
    LimboChainOp.sys_validation_status,
    LimboChainOp.app_validation_status
FROM LimboChainOp
WHERE
    LimboChainOp.abandoned_at IS NOT NULL
    OR LimboChainOp.sys_validation_status = 2
    OR (
        LimboChainOp.sys_validation_status = 1
        AND LimboChainOp.app_validation_status IN (1, 2)
    )
ORDER BY LimboChainOp.when_received
```

**Workflow Steps:**

1. **Query Completed Ops**
   - Select ops where validation was abandoned, sys validation was rejected, or both sys and app validation reached a terminal state.
   - Accepted, rejected, and abandoned ops are all integrated (removed from limbo).

2. **Move Op to ChainOp Table**
   - Start a write transaction for each op.
   - Execute the following insert:
   ```sql
   INSERT INTO ChainOp (
       hash,
       op_type,
       action_hash,
       basis_hash,
       storage_center_loc,
       validation_status,
       locally_validated,
       when_received,
       when_integrated,
       serialized_size
   )
   SELECT
       hash,
       op_type,
       action_hash,
       basis_hash,
       storage_center_loc,
       CASE
           WHEN sys_validation_status = 1 AND app_validation_status = 1 THEN 1
           ELSE 2
       END,
       TRUE,  -- locally_validated
       when_received,
       unixepoch(),
       serialized_size  -- calculated when op arrived
   FROM LimboChainOp
   WHERE hash = :op_hash
   ```

3. **Update `Action` with Record Validity**
   - Aggregate `record_validity` from all ops for this action using a single query
   - **Rules (applied in SQL):**
     - If ANY op for this action is rejected: `record_validity = 2`
     - If at least one op is accepted and none rejected: `record_validity = 1`
   ```sql
   UPDATE Action
   SET record_validity = (
       SELECT CASE
           WHEN COUNT(CASE WHEN validation_status = 2 THEN 1 END) > 0 THEN 2
           WHEN COUNT(CASE WHEN validation_status = 1 THEN 1 END) > 0 THEN 1
           ELSE NULL
       END
       FROM ChainOp
       WHERE action_hash = :action_hash
   )
   WHERE hash = :action_hash
   ```

   Note: For network-received ops, `Action` rows are inserted during incoming op processing with `record_validity = NULL` and updated here once an op integrates. For self-authored ops, `Action` rows are inserted at authoring time with `record_validity = 1` and the corresponding `ChainOp` rows bypass limbo but still go through integration to populate index tables and register in the DHT model. `Entry` rows are also inserted at incoming/authoring time (if the op carries a public entry), but `Entry` has no validity field — no entry-specific step is needed during integration.

4. **Update Index Tables** (if applicable)
   - If the action is a `CreateLink` and the op is accepted, insert into `Link` table:
   ```sql
   INSERT INTO Link (action_hash, base_hash, zome_index, link_type, tag)
   SELECT
       Action.hash,
       :base_hash,      -- extracted from action_data during integration
       :zome_index,     -- extracted from action_data during integration
       :link_type,      -- extracted from action_data during integration
       :tag             -- extracted from action_data during integration
   FROM Action
   WHERE Action.hash = :action_hash
       AND Action.record_validity = 1
       AND Action.action_type = 7 -- ActionType::CreateLink
   ```
   - If the action is a `CreateLink` and the op is rejected, ensure no row exists in `Link` table for this action:
   ```sql
   DELETE FROM Link
   WHERE action_hash = :action_hash
   AND :action_hash IN (
       SELECT hash FROM Action
       WHERE hash = :action_hash AND record_validity = 2
   );
   ```
   - If the action is a `DeleteLink` and the op is accepted, insert into `DeletedLink` table:
   ```sql
   INSERT INTO DeletedLink (action_hash, create_link_hash)
   SELECT
       Action.hash,
       :create_link_hash  -- extracted from action_data during integration
   FROM Action
   WHERE Action.hash = :action_hash
       AND Action.record_validity = 1
       AND Action.action_type = 8 -- ActionType::DeleteLink
   ```
   - If the action is a `DeleteLink` and the op is rejected, ensure no row exists in `DeletedLink` table for this action:
   ```sql
   DELETE FROM DeletedLink
   WHERE action_hash = :action_hash
   AND :action_hash IN (
       SELECT hash FROM Action
       WHERE hash = :action_hash AND record_validity = 2
   );
   ```
   - If the action is an `Update` and the op is accepted, insert into `UpdatedRecord` table:
   ```sql
   INSERT INTO UpdatedRecord (action_hash, original_action_hash, original_entry_hash)
   SELECT
       Action.hash,
       :original_action_hash,  -- extracted from action_data during integration
       :original_entry_hash    -- extracted from action_data during integration
   FROM Action
   WHERE Action.hash = :action_hash
       AND Action.record_validity = 1
       AND Action.action_type = 5 -- ActionType::Update
   ```
   - If the action is an `Update` and the op is rejected, ensure no row exists in `UpdatedRecord` table:
   ```sql
   DELETE FROM UpdatedRecord
   WHERE action_hash = :action_hash
   AND :action_hash IN (
       SELECT hash FROM Action
       WHERE hash = :action_hash AND record_validity = 2
   );
   ```
   - If the action is a `Delete` and the op is accepted, insert into `DeletedRecord` table:
   ```sql
   INSERT INTO DeletedRecord (action_hash, deletes_action_hash, deletes_entry_hash)
   SELECT
       Action.hash,
       :deletes_action_hash,  -- extracted from action_data during integration
       :deletes_entry_hash    -- extracted from action_data during integration
   FROM Action
   WHERE Action.hash = :action_hash
       AND Action.record_validity = 1
       AND Action.action_type = 6 -- ActionType::Delete
   ```
   - If the action is a `Delete` and the op is rejected, ensure no row exists in `DeletedRecord` table:
   ```sql
   DELETE FROM DeletedRecord
   WHERE action_hash = :action_hash
   AND :action_hash IN (
       SELECT hash FROM Action
       WHERE hash = :action_hash AND record_validity = 2
   );
   ```

5. **Delete from `LimboChainOp`**
   ```sql
   DELETE FROM LimboChainOp WHERE hash = :op_hash
   ```

6. **Commit Transaction**
    - Commit the write transaction to finalize changes for this op.

7. **Send Validation Receipt** (if required)
   - If op came from network and `require_receipt = true`
   - Send a signed validation receipt back to the author

### Record Validity Aggregation

**Rules:**
1. A record is **rejected** (`record_validity = 2`) if ANY known ops for it are rejected.
2. A record is **accepted** (`record_validity = 1`) if at least one known op for it is accepted and no known ops are rejected.
3. Self-authored records are inserted with `record_validity = 1` (pre-validated at authoring time).
4. Network-received records in limbo have a `NULL` value for `record_validity`. After integration of the first op for a record, `record_validity` is `1` (accepted) or `2` (rejected).

The record validity is first set at authoring time for self-authored records, or at the time of integrating the first of its ops for network-received records. If later op integrations find an op to be rejected, the record validity is updated to `2` (rejected). Due to the sharding model, a validator may not have all ops for a record — they hold specific op types over specific ranges. This aggregated status is stored with the record itself, eliminating the need for complex joins during queries.

### Record Validity Correction

TODO: It is a future piece of work to define this logic. When implemented, this logic must also clean up index tables (`Link`, `DeletedLink`, `UpdatedRecord`, `DeletedRecord`) when records are invalidated.

## Query Patterns

### Get Record by Hash

Retrieve a complete record (action and entry) from the DHT by action hash.

```sql
SELECT Action.*, Entry.*
FROM Action
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE Action.hash = ?
  AND Action.record_validity = 1
```

### Get Entry by Hash

Retrieve an entry from the DHT using the validation status from any action that references it.

```sql
SELECT Entry.*, Action.*
FROM Entry
JOIN Action ON Action.entry_hash = Entry.hash
WHERE Entry.hash = ?
  AND Action.record_validity = 1
LIMIT 1
```

Justification for *any*: An entry can be referenced by multiple actions. If at least one action referencing the entry is accepted, the entry is considered accepted to be served to the network or used.

### Get Links

Query all non-deleted links from a base hash using the `Link` index table populated at integration time.

Find non-deleted links from base:

```sql
SELECT Link.*, Action.*
FROM Link
JOIN Action ON Link.action_hash = Action.hash
LEFT JOIN DeletedLink ON Link.action_hash = DeletedLink.create_link_hash
WHERE Link.base_hash = ?
  AND DeletedLink.create_link_hash IS NULL
  AND Action.record_validity = 1
ORDER BY Action.timestamp, Link.action_hash
```

Additional filters can be applied in SQL or application code:
- link_type: `WHERE Link.link_type = ?`
- zome_index: `WHERE Link.zome_index = ?`
- tag prefix: `WHERE Link.tag >= ? AND Link.tag < ?` (bytewise comparison)
- author: `WHERE Action.author = ?`
- timestamp bounds: `WHERE Action.timestamp BETWEEN ? AND ?`

### Count Links

Count non-deleted links matching query criteria using the `Link` index table.

Count non-deleted links:

```sql
SELECT COUNT(*)
FROM Link
LEFT JOIN DeletedLink ON Link.action_hash = DeletedLink.create_link_hash
JOIN Action ON Link.action_hash = Action.hash
WHERE Link.base_hash = ?
  AND DeletedLink.create_link_hash IS NULL
  AND Action.record_validity = 1
```

Additional filters (link_type, tag prefix, author, timestamp) applied as needed.

### Get Agent Activity

Query an agent's chain activity with flexible filtering for action types, entry types, and validation status.

```sql
SELECT Action.*
FROM Action
WHERE Action.author = ?
  AND Action.record_validity IS NOT NULL
ORDER BY Action.seq
```

Application code filters for:
- Specific action_type values
- Entry types (via action_data deserialization)
- Sequence ranges
- Include accepted/rejected/warrants based on GetActivityOptions

### Must Get Agent Activity

Deterministic hash-bounded query for agent activity used in countersigning scenarios.

Query with sequence bounds:

```sql
SELECT Action.*
FROM Action
WHERE Action.author = ?
  AND Action.seq BETWEEN ? AND ?
  AND Action.record_validity IS NOT NULL
ORDER BY Action.seq
```

Application code verifies that there are no gaps in sequence numbers (complete chain segment).

### Query Authored Chain with `ChainQueryFilter`

Query the authored source chain with filtering criteria from `ChainQueryFilter`. The chain is ordered by sequence and typically queried by walking the chain. Both public and private entries are returned for the author's own chain by joining to both `Entry` and `PrivateEntry`.

Base query with common filters:

```sql
SELECT Action.*, COALESCE(Entry.blob, PrivateEntry.blob) as entry_blob
FROM Action
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
LEFT JOIN PrivateEntry ON Action.entry_hash = PrivateEntry.hash
    AND PrivateEntry.author = :author
WHERE Action.author = :author
  AND Action.seq BETWEEN :start_seq AND :end_seq
  AND Action.action_type IN (4, 5, ...) -- ActionType::Create, ActionType::Update, etc.
  AND Action.entry_hash IN (...)
ORDER BY Action.seq ASC
```

**Handling Different Range Types:**

**Unbounded**: No sequence filtering

```sql
SELECT Action.*, COALESCE(Entry.blob, PrivateEntry.blob) as entry_blob
FROM Action
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
LEFT JOIN PrivateEntry ON Action.entry_hash = PrivateEntry.hash
    AND PrivateEntry.author = :author
WHERE Action.author = :author
ORDER BY Action.seq ASC
```

**ActionSeqRange(start, end)**: Filter by sequence numbers

```sql
SELECT Action.*, COALESCE(Entry.blob, PrivateEntry.blob) as entry_blob
FROM Action
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
LEFT JOIN PrivateEntry ON Action.entry_hash = PrivateEntry.hash
    AND PrivateEntry.author = :author
WHERE Action.author = :author
  AND Action.seq BETWEEN :start_seq AND :end_seq
ORDER BY Action.seq ASC
```

**ActionHashRange(start_hash, end_hash)**: Hash-bounded range with fork disambiguation
- Query all actions
- Walk backwards from `end_hash` following `prev_hash` until reaching `start_hash`
- Application code performs the chain traversal (think this cannot be done efficiently in SQL, experiment with recursive queries at implementation time)

**ActionHashTerminated(end_hash, n)**: Hash-terminated with N preceding records
- Query all actions
- Walk backwards from `end_hash` following `prev_hash` for N steps
- Application code performs the chain traversal

**Entry Type Filtering:**

Entry type filtering requires deserializing `action_data` to extract the `entry_type` field. This should be handled in application code after fetching matching actions,

### Cap Grant and Claim Lookups

Cap grant/claim lookups use dedicated tables for direct access without chain scans:

Find cap grant by access type (direct lookup, join for other fields):

```sql
SELECT cg.*, Action.*, PrivateEntry.blob as entry_blob
FROM CapGrant cg
JOIN Action ON cg.action_hash = Action.hash
JOIN PrivateEntry ON Action.entry_hash = PrivateEntry.hash
    AND PrivateEntry.author = :author
WHERE cg.cap_access = ?
  AND Action.author = :author
ORDER BY Action.seq;
```

Find all grants by an author:

```sql
SELECT cg.*, Action.*, PrivateEntry.blob as entry_blob
FROM CapGrant cg
JOIN Action ON cg.action_hash = Action.hash
JOIN PrivateEntry ON Action.entry_hash = PrivateEntry.hash
    AND PrivateEntry.author = :author
WHERE Action.author = :author
ORDER BY Action.seq;
```

Find grants by tag:

```sql
SELECT cg.*, Action.*, PrivateEntry.blob as entry_blob
FROM CapGrant cg
JOIN Action ON cg.action_hash = Action.hash
JOIN PrivateEntry ON Action.entry_hash = PrivateEntry.hash
    AND PrivateEntry.author = :author
WHERE cg.tag = ?
  AND Action.author = :author
ORDER BY Action.seq;
```

Find cap claims by grantor:

```sql
SELECT * FROM CapClaim
WHERE author = :author
  AND grantor = ?
ORDER BY id;
```

Find cap claims by tag:

```sql
SELECT * FROM CapClaim
WHERE author = :author
  AND tag = ?
ORDER BY id;
```

Chain traversal (no BLOB deserialization needed):

```sql
SELECT hash, seq, prev_hash, timestamp, action_type
FROM Action
WHERE author = ?
ORDER BY seq;
```

Full action retrieval (deserialize BLOB for details):

```sql
SELECT * FROM Action WHERE hash = ?;
```

Then deserialize `action_data` BLOB in application.

### Find Updates for a Create Action

Retrieve all `Update` actions that reference a specific `Create` action. Updates form a chain where each `Update` references either the original `Create` or another `Update`.

Uses the `UpdatedRecord` index table populated during integration.

```sql
-- Find direct updates (one hop)
SELECT Action.*, Entry.*
FROM UpdatedRecord
JOIN Action ON UpdatedRecord.action_hash = Action.hash
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE UpdatedRecord.original_action_hash = ?
  AND Action.record_validity = 1
ORDER BY Action.seq

-- Find full update chain (recursive)
WITH RECURSIVE update_chain AS (
  -- Base case: direct updates of the create
  SELECT
    UpdatedRecord.action_hash as hash,
    Action.author,
    Action.seq,
    Action.timestamp,
    Action.entry_hash,
    1 as depth
  FROM UpdatedRecord
  JOIN Action ON UpdatedRecord.action_hash = Action.hash
  WHERE UpdatedRecord.original_action_hash = ?
    AND Action.record_validity = 1

  UNION ALL

  -- Recursive case: updates of updates
  SELECT
    ur.action_hash,
    a.author,
    a.seq,
    a.timestamp,
    a.entry_hash,
    uc.depth + 1
  FROM UpdatedRecord ur
  JOIN Action a ON ur.action_hash = a.hash
  INNER JOIN update_chain uc ON ur.original_action_hash = uc.hash
  WHERE a.record_validity = 1
)
SELECT uc.*, Entry.*
FROM update_chain uc
LEFT JOIN Entry ON uc.entry_hash = Entry.hash
ORDER BY depth, seq
```

**Use Cases:**
- Get the latest version of a record
- Show update history to users
- Traverse the update chain to find all versions

**Application Logic:**
- Latest version: take the last update in the chain, or the original if no updates exist
- Update graph: if multiple updates reference the same action, the chain branches

### Find Deletes for an Action

Retrieve all `Delete` actions that reference a specific action (`Create` or `Update`).

Uses the `DeletedRecord` index table populated during integration.

```sql
-- Find deletes for a specific action
SELECT Action.*
FROM DeletedRecord
JOIN Action ON DeletedRecord.action_hash = Action.hash
WHERE DeletedRecord.deletes_action_hash = ?
  AND Action.record_validity = 1
ORDER BY Action.seq
```

**Use Cases:**
- Check if a record has been deleted
- Show who deleted a record and when
- Filter out deleted content from queries

### Find All Deletes for a Record (Create + Update Chain)

Retrieve all `Delete` actions that reference either the original `Create` or any `Update` in its chain.

Uses the `UpdatedRecord` and `DeletedRecord` index tables.

```sql
-- Find all actions in the record's lifecycle (create + updates)
WITH RECURSIVE record_chain AS (
  -- Base case: the original create/update
  SELECT hash, 0 as depth
  FROM Action
  WHERE hash = ?

  UNION ALL

  -- Recursive case: updates of this action
  SELECT UpdatedRecord.action_hash, rc.depth + 1
  FROM UpdatedRecord
  JOIN Action ON UpdatedRecord.action_hash = Action.hash
  INNER JOIN record_chain rc ON UpdatedRecord.original_action_hash = rc.hash
  WHERE Action.record_validity = 1
)
-- Find all deletes for any action in the chain
SELECT DISTINCT Action.*
FROM DeletedRecord
JOIN Action ON DeletedRecord.action_hash = Action.hash
WHERE Action.record_validity = 1
  AND DeletedRecord.deletes_action_hash IN (SELECT hash FROM record_chain)
ORDER BY Action.seq
```

**Use Cases:**
- Determine if any version of a record has been deleted
- Show complete deletion history
- Filter records that have been deleted in any version

**Rationale:**
In Holochain, deleting an `Update` action doesn't automatically delete the original `Create`. Applications may want to consider a record "deleted" if ANY version is deleted, or only if ALL versions are deleted. This query returns all deletes; application logic decides how to interpret them.

### Get Live Records (Filter Out Deleted)

Query for records that haven't been deleted. A record is "live" if there are no `Delete` actions referencing it.

Uses the `DeletedRecord` index table.

```sql
-- Get live records by excluding those with deletes
SELECT Action.*, Entry.*
FROM Action
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE Action.action_type = 4 -- ActionType::Create
  AND Action.record_validity = 1
  -- Exclude creates that have been deleted
  AND NOT EXISTS (
    SELECT 1 FROM DeletedRecord
    JOIN Action AS DeleteAction ON DeletedRecord.action_hash = DeleteAction.hash
    WHERE DeleteAction.record_validity = 1
      AND DeletedRecord.deletes_action_hash = Action.hash
  )
ORDER BY Action.seq
```

**Use Cases:**
- List all active/live records
- Filter query results to exclude deleted content
- Count non-deleted records

**Application Variations:**
- **Strict live filter**: Exclude records where ANY version (create or update) has been deleted
- **Permissive live filter**: Only exclude records where ALL versions have been deleted
- **Latest live version**: Get the most recent update that hasn't been deleted

### Get Record Details (Complete Lifecycle)

Retrieve a complete record with all updates and deletes, letting the application decide how to resolve the current state.

Uses the `UpdatedRecord` and `DeletedRecord` index tables.

```sql
WITH RECURSIVE
update_chain AS (
  -- Base case: direct updates of the create
  SELECT
    UpdatedRecord.action_hash AS hash,
    Action.author,
    Action.seq,
    Action.prev_hash,
    Action.timestamp,
    Action.action_type,
    Action.action_data,
    Action.entry_hash,
    Action.record_validity,
    1 AS depth
  FROM UpdatedRecord
  JOIN Action ON UpdatedRecord.action_hash = Action.hash
  WHERE UpdatedRecord.original_action_hash = ?
    AND Action.record_validity = 1

  UNION ALL

  -- Recursive case: updates of updates
  SELECT
    ur.action_hash,
    a.author,
    a.seq,
    a.prev_hash,
    a.timestamp,
    a.action_type,
    a.action_data,
    a.entry_hash,
    a.record_validity,
    uc.depth + 1
  FROM UpdatedRecord ur
  JOIN Action a ON ur.action_hash = a.hash
  INNER JOIN update_chain uc ON ur.original_action_hash = uc.hash
  WHERE a.record_validity = 1
),
record_chain AS (
  -- Base case: the original create/update
  SELECT hash, 0 AS depth
  FROM Action
  WHERE hash = ?

  UNION ALL

  -- Recursive case: updates of this action
  SELECT ur.action_hash, rc.depth + 1
  FROM UpdatedRecord ur
  JOIN Action ON ur.action_hash = Action.hash
  INNER JOIN record_chain rc ON ur.original_action_hash = rc.hash
  WHERE Action.record_validity = 1
)
-- Get the original action
SELECT
  'original' AS record_type,
  Action.hash, Action.author, Action.seq, Action.prev_hash,
  Action.timestamp, Action.action_type, Action.action_data,
  Action.entry_hash, Action.record_validity,
  Entry.blob AS entry_blob,
  0 AS depth
FROM Action
LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE Action.hash = ?
  AND Action.record_validity = 1

UNION ALL

-- Get all updates in the chain
SELECT
  'update' AS record_type,
  uc.hash, uc.author, uc.seq, uc.prev_hash,
  uc.timestamp, uc.action_type, uc.action_data,
  uc.entry_hash, uc.record_validity,
  Entry.blob AS entry_blob,
  uc.depth
FROM update_chain uc
LEFT JOIN Entry ON uc.entry_hash = Entry.hash

UNION ALL

-- Get all deletes for any action in the chain
SELECT
  'delete' AS record_type,
  Action.hash, Action.author, Action.seq, Action.prev_hash,
  Action.timestamp, Action.action_type, Action.action_data,
  Action.entry_hash, Action.record_validity,
  NULL AS entry_blob,
  0 AS depth
FROM DeletedRecord
JOIN Action ON DeletedRecord.action_hash = Action.hash
WHERE Action.record_validity = 1
  AND DeletedRecord.deletes_action_hash IN (SELECT hash FROM record_chain)

ORDER BY depth, seq
```

**Return Structure (Application Layer):**

```rust
pub struct RecordDetails {
    /// The original action (`Create` or `Update`)
    pub original: Record,
    /// All updates in chronological order
    pub updates: Vec<Record>,
    /// All deletes affecting this record or its updates
    pub deletes: Vec<SignedActionHashed>,
}

impl RecordDetails {
    /// Check if this record is live (not deleted)
    pub fn is_live(&self) -> bool {
        self.deletes.is_empty()
    }

    /// Get the latest version (last update or original if no updates)
    pub fn latest_version(&self) -> &Record {
        self.updates.last().unwrap_or(&self.original)
    }

    /// Get the latest live version (excluding deleted updates)
    pub fn latest_live_version(&self) -> Option<&Record> {
        let deleted_hashes: HashSet<_> = self.deletes
            .iter()
            .map(|d| d.action().deletes_address())
            .collect();

        // Check updates in reverse order
        for update in self.updates.iter().rev() {
            if !deleted_hashes.contains(&update.action_address()) {
                return Some(update);
            }
        }

        // Check original
        if !deleted_hashes.contains(&self.original.action_address()) {
            Some(&self.original)
        } else {
            None
        }
    }
}
```

**Use Cases:**
- Display complete record history to users
- Let application implement custom resolution logic
- Show "edited" and "deleted" indicators in UI
- Implement conflict resolution for forked update chains

**Notes:**
- This query may return substantial data for records with many updates/deletes
- Consider pagination for records with long histories
- Application layer decides interpretation: "deleted" might mean ANY delete, or ALL versions deleted
- Update chains can branch if multiple updates reference the same action (application handles resolution)

### Cache Pruning Strategy

TODO: Design how to track and prune cached ops when storage pressure occurs.

## Warrant Handling

Warrants use parallel limbo and integrated tables to chain ops (`LimboWarrant` → `Warrant`), but with a simpler validation flow: warrants have sys validation only — there is no app validation step.

### Warrant Processing Flow

1. **Incoming DHT Ops Workflow**
   - Warrant op arrives as `DhtOp::WarrantOp(warrant)`
   - Hash verification and counterfeit checks performed
   - Inserted into `LimboWarrant` (no action or entry insertion)

2. **Warrant Sys Validation Workflow**
   - Warrant validation depends on warrant type:

   **ChainIntegrityWarrant**: Proves an author broke chain rules
   - Stays in `LimboWarrant` until the warranted action is fetched and validated
   - If warranted action is rejected: warrant is accepted (proves the claim)
   - If warranted action is accepted: warrant is rejected (false claim)

   **ChainForkWarrant**: Proves an author has forked their chain
   - Stays in `LimboWarrant` until both forked actions are fetched and checked
   - If both forked actions are at the same sequence number: warrant is accepted (proves fork)
   - If the forked actions do not match the fork condition: warrant is rejected (false claim)

   After validation, update `LimboWarrant.sys_validation_status` to `0` (accepted) or `1` (rejected).

3. **Warrant Integration Workflow**
   - Query `LimboWarrant` for ops with a terminal state:
   ```sql
   SELECT * FROM LimboWarrant
   WHERE abandoned_at IS NOT NULL
      OR sys_validation_status IN (1, 2)
   ORDER BY when_received
   ```
   - For accepted warrants, insert into `Warrant`:
   ```sql
   INSERT INTO Warrant (hash, author, timestamp, warrantee, proof, storage_center_loc)
   SELECT hash, author, timestamp, warrantee, proof, storage_center_loc
   FROM LimboWarrant
   WHERE hash = :warrant_hash
     AND sys_validation_status = 1
   ```
   - Delete from `LimboWarrant` and commit.

### Warrant Query Patterns

Accepted warrants can be queried from the `Warrant` table:

Find all warrants against a specific agent:

```sql
SELECT * FROM Warrant
WHERE warrantee = ?
ORDER BY timestamp DESC;
```

Check if a specific warrant exists:

```sql
SELECT EXISTS(
    SELECT 1 FROM Warrant WHERE hash = ?
);
```

## Data Integrity Invariants

The system maintains these invariants:

1. **No chain op exists in both `LimboChainOp` and `ChainOp` simultaneously; no warrant exists in both `LimboWarrant` and `Warrant` simultaneously**
2. **Self-authored ops are never in limbo**: They are inserted directly into `ChainOp`/`Warrant` at authoring time, but still go through integration to populate index tables
3. **Every `ChainOp` has a definite `validation_status` (never `NULL`)**
4. **`Action.record_validity` is `1` for self-authored records, `NULL` for pending network-received records, `1` (accepted) or `2` (rejected) after integration**
5. **Network-received ops are moved from limbo to integrated tables atomically with `record_validity` updates**
6. **Rejected ops in `ChainOp` cause their records to be marked 'rejected'**
7. **Dependencies are resolved before validation proceeds**
8. **Queries for validated data always check `record_validity IS NOT NULL` or `record_validity = 1`**
9. **No validation status uses 0**: Accepted=1, rejected=2 across all status fields. A default or uninitialized 0 value is never treated as a valid state
10. **Private entries exist only in `PrivateEntry`, never in `Entry`**: The `private_entry` flag on `Action` determines which table receives the entry at write time
