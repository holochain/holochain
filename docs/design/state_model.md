# Data Logic Design Reference
# Data State Model

## Overview

The Holochain data storage and validation architecture provides:

1. Ops as the unit of validation with aggregated validity status for records
2. Validation staging tables to separate pending data from validated data
3. Unified data querying without separate cache database
4. Distinct schemas for authored and DHT databases, with staging tables in DHT
5. Direct data queries without complex joins

## Architecture

### Core Principles

1. **Ops are the unit of validation**: All validation happens at the op level
2. **Records aggregate op validity**: A record's validity is derived from its constituent ops
3. **Validation staging isolates pending data**: Unvalidated ops stay in staging tables until validated
4. **Distinct schemas per database type**: Authored and DHT databases have schemas tailored to their needs
5. **Unified data storage**: DHT database serves both obligated and cached data, distinguished by arc coverage
6. **Clear state transitions**: Data moves through well-defined states with no ambiguity

### Database Structure

#### 1. Authored Database
**Purpose**: Store an agent's own authored chain data

```sql
-- Authored Actions
CREATE TABLE Action (
    hash BLOB PRIMARY KEY,
    author BLOB NOT NULL,
    seq INTEGER NOT NULL,
    prev_hash BLOB,
    timestamp INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    action_data BLOB -- Serialized ActionData enum
);

-- Index tables for queryable action-specific fields
CREATE TABLE ActionEntry (
    action_hash BLOB PRIMARY KEY,
    entry_hash BLOB NOT NULL,
    entry_type TEXT,
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- Authored Entries
CREATE TABLE Entry (
    hash BLOB PRIMARY KEY,
    blob BLOB NOT NULL,
    -- Entry-specific fields for CapClaim/CapGrant
);

-- Authored Ops (for publishing)
CREATE TABLE AuthoredOp (
    hash BLOB PRIMARY KEY,
    action_hash BLOB NOT NULL,
    op_type TEXT NOT NULL,
    basis_hash BLOB NOT NULL,
    
    when_published INTEGER,
    publish_attempts INTEGER DEFAULT 0,
    
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);
```

#### 2. DHT Database with Staging Tables
**Purpose**: Store validated DHT data and ops pending validation

```sql
-- Staging tables for ops being validated
CREATE TABLE StagingOp (
    hash BLOB PRIMARY KEY,
    op_type TEXT NOT NULL,
    action_hash BLOB NOT NULL,
    basis_hash BLOB NOT NULL,
    
    -- Validation tracking
    validation_stage TEXT NOT NULL, -- 'pending_sys', 'pending_app', 'complete'
    sys_validation_status TEXT,     -- NULL, 'valid', 'rejected', 'abandoned'
    app_validation_status TEXT,     -- NULL, 'valid', 'rejected', 'abandoned'
    
    -- Dependencies for validation ordering
    dependency1 BLOB,
    dependency2 BLOB,
    
    -- Timing
    when_received INTEGER NOT NULL,
    validation_attempts INTEGER DEFAULT 0,
    last_validation_attempt INTEGER,
    
    -- The staged action and entry data
    action_blob BLOB NOT NULL,
    entry_blob BLOB
);

-- Track validation receipts
CREATE TABLE ValidationReceipt (
    hash BLOB PRIMARY KEY,
    op_hash BLOB NOT NULL,
    validator BLOB NOT NULL,
    signature BLOB NOT NULL,
    when_received INTEGER NOT NULL,
    
    FOREIGN KEY(op_hash) REFERENCES StagingOp(hash) ON DELETE CASCADE
);
```

-- Validated tables in DHT database
CREATE TABLE DhtAction (
    hash BLOB PRIMARY KEY,
    author BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    action_data BLOB NOT NULL, -- Serialized ActionData enum
    
    -- Record validity (aggregated from ops)
    record_validity TEXT NOT NULL -- 'valid', 'rejected', 'abandoned'
);

-- Index table for DHT action queries
CREATE TABLE DhtActionEntry (
    action_hash BLOB PRIMARY KEY,
    entry_hash BLOB NOT NULL,
    entry_type TEXT,
    FOREIGN KEY(action_hash) REFERENCES DhtAction(hash)
);

-- Validated Entries in DHT  
CREATE TABLE DhtEntry (
    hash BLOB PRIMARY KEY,
    blob BLOB NOT NULL,
    
    -- Entry validity (aggregated from ops)
    validity TEXT NOT NULL -- 'valid', 'rejected', 'abandoned'
);

-- Validated Ops in DHT
CREATE TABLE DhtOp (
    hash BLOB PRIMARY KEY,
    op_type TEXT NOT NULL,
    action_hash BLOB NOT NULL,
    basis_hash BLOB NOT NULL,
    storage_center_loc INTEGER NOT NULL,
    
    -- Final validation result
    validation_status TEXT NOT NULL, -- 'valid', 'rejected', 'abandoned'
    
    -- Integration tracking
    when_integrated INTEGER NOT NULL,
    
    -- Publishing/gossip tracking
    last_publish_time INTEGER,
    receipts_complete BOOLEAN DEFAULT FALSE,
    
    FOREIGN KEY(action_hash) REFERENCES DhtAction(hash)
);

-- Links (derived from ops, for efficient querying)
CREATE TABLE DhtLink (
    create_link_hash BLOB PRIMARY KEY,
    base_hash BLOB NOT NULL,
    target_hash BLOB NOT NULL,
    zome_index INTEGER NOT NULL,
    link_type INTEGER NOT NULL,
    tag BLOB,
    author BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    
    -- Link status
    is_deleted BOOLEAN DEFAULT FALSE,
    
    FOREIGN KEY(create_link_hash) REFERENCES DhtAction(hash)
);
```

### Validation Flow

```
1. Op arrives (from author or network)
   └─> Insert into StagingOp

2. Sys Validation Workflow
   ├─> Check dependencies in DhtAction/DhtEntry
   ├─> Perform sys validation checks
   └─> Update sys_validation_status in StagingOp

3. App Validation Workflow (if sys valid)
   ├─> Run WASM validation
   └─> Update app_validation_status in StagingOp

4. Integration Workflow (if all valid)
   ├─> Move op from StagingOp to DhtOp
   ├─> Insert/update DhtAction with aggregated validity
   ├─> Insert/update DhtEntry with aggregated validity
   └─> Delete from StagingOp
```

### Record Validity Aggregation

**Rules:**
1. A record is **INVALID** if ANY known ops are rejected
2. A record is **VALID** if at least one known op is valid and no known ops are rejected
3. A record is **ABANDONED** if all known ops are abandoned
4. A record's validity is computed on integration, not on query
5. Validators may have partial views due to sharding - validity is based only on known ops

The record validity is determined at the time of integration by examining all known ops associated with the record. Due to the sharding model, a validator may not have all ops for a record - they hold specific op types over specific ranges. This aggregated status is stored with the record itself, eliminating the need for complex joins during queries.

**Note on Partial Views:**
Since validators hold shards (specific op types over specific ranges), they make validity decisions based on the ops they know about. A record is considered valid if any of its known ops are valid and none are known to be invalid. The absence of some ops does not make a record pending or invalid.

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
SELECT * FROM DhtEntry
WHERE hash = ?
  AND validity = 'valid'
```

### Get Links

```sql
SELECT *
FROM DhtLink
WHERE base_hash = ?
  AND is_deleted = FALSE
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
Ops are inserted into StagingOp table with validation_stage='pending_sys'

### Sys Validation Workflow  
Updates sys_validation_status in StagingOp, triggers app validation or abandonment

### App Validation Workflow
Updates app_validation_status in StagingOp, triggers integration if valid

### Integration Workflow
Moves validated ops from StagingOp to DhtOp, updates record validity in DhtAction/DhtEntry

### Publish DHT Ops Workflow
Queries DhtOp for ops to publish (all ops in DhtOp are integrated)

### Validation Receipt Workflow
Inserts receipts linked to StagingOp, updates receipts_complete when moved to DhtOp

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
   - Stays in StagingOp until warranted action is fetched and validated
   - If warranted action is rejected, warrant moves to DhtOp as valid
   - If warranted action is valid, warrant is rejected

2. **Warrant Validation Dependencies**: 
   - Warrants can have up to 2 dependencies (the actions being warranted)
   - These must be resolved before warrant validation can complete
   - Dependencies tracked in dependency1 and dependency2 fields of StagingOp

## Key Operations

### Mutations

1. **insert_network_op** (from network)
   - Insert into StagingOp with validation_stage='pending_sys'

2. **author_action** (local authoring)
   - Insert into authored Action/Entry tables
   - Create AuthoredOp records
   - Validate locally before committing
   - On validation failure, rollback without affecting chain

3. **publish_authored_op** (after local validation)
   - Mark AuthoredOp as published (set when_published)
   - Op enters network propagation

4. **set_validation_status** (after validation)
   - Update StagingOp.sys/app_validation_status

5. **set_when_integrated** (after all validation)
   - Move from StagingOp to DhtOp
   - Update record validity in DhtAction/DhtEntry

6. **set_receipts_complete** (after enough receipts)
   - Update DhtOp.receipts_complete

### Queries

1. **get_record** - Query DhtAction + DhtEntry by record_validity
2. **get_entry** - Direct query of DhtEntry by validity
3. **get_links** - Direct query of DhtLink table
4. **agent_activity** - Query DhtAction by author and record_validity
5. **validation_limbo** - Query StagingOp by validation_stage

## Data Integrity Invariants

The system maintains these invariants:

1. **No op exists in both StagingOp and DhtOp simultaneously**
2. **Every DhtOp has a definite validation_status (never NULL)**
3. **Every DhtAction has a computed record_validity**
4. **Ops move from staging to DHT atomically with record updates**
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
    hash BLOB PRIMARY KEY,
    author BLOB NOT NULL,
    seq INTEGER NOT NULL,
    prev_hash BLOB,
    timestamp INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    action_data BLOB -- Serialized ActionData enum
);

-- Index tables for queryable action-specific fields

-- For efficient capability grant/claim queries
CREATE TABLE CapabilityGrant (
    action_hash BLOB PRIMARY KEY,
    grantor BLOB NOT NULL,
    grantee BLOB NOT NULL,
    cap_secret BLOB,
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

CREATE TABLE CapabilityClaim (
    action_hash BLOB PRIMARY KEY,
    grantor BLOB NOT NULL,
    cap_secret BLOB NOT NULL,
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- For entry-related queries
CREATE TABLE ActionEntry (
    action_hash BLOB PRIMARY KEY,
    entry_hash BLOB NOT NULL,
    entry_type TEXT,
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
);

-- For link queries (already separate in LinkOp tables)
-- Links are typically queried through dedicated link tables
```

### Rust Structure
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
    CloseChain(CloseChainData),
    OpenChain(OpenChainData),
}

/// Lightweight reference for queries
pub struct ActionRef {
    pub hash: ActionHash,
    pub action_type: ActionType,
    pub header: ActionHeader,
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
pub struct CloseChainData {
    pub new_dna_hash: DnaHash,
}
pub struct OpenChainData {
    pub previous_dna_hash: DnaHash,
}
```

### Query Patterns

```sql
-- Chain traversal (no BLOB deserialization needed)
SELECT hash, seq, prev_hash, timestamp 
FROM Action 
WHERE author = ? 
ORDER BY seq;

-- Find capability grants for an agent
SELECT a.*, cg.* 
FROM Action a
JOIN CapabilityGrant cg ON a.hash = cg.action_hash
WHERE cg.grantee = ?;

-- Get entries by type (using index table)
SELECT a.*, ae.entry_hash
FROM Action a
JOIN ActionEntry ae ON a.hash = ae.action_hash
WHERE ae.entry_type = ? AND a.author = ?;

-- Full action retrieval (deserialize BLOB for complete data)
SELECT * FROM Action WHERE hash = ?;
-- Then deserialize action_data BLOB in application layer
```

### Benefits of This Approach

1. **Minimal redundancy** - Common fields stored once
2. **Simple core schema** - Main table remains clean
3. **Selective indexing** - Only queryable fields get index tables
4. **Efficient chain operations** - No BLOB deserialization for traversal
5. **Flexible querying** - Index tables enable specific queries without full scans
