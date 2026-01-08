# Data Logic Design Reference

## Overview

The Holochain data storage and validation architecture provides:

1. Ops as the unit of validation with aggregated validity status for records
2. A validation staging area to separate pending data from validated data
3. Unified data querying without separate cache database
4. Distinct schemas for authored, DHT, and validation staging databases
5. Direct data queries without complex joins

## Architecture

### Core Principles

1. **Ops are the unit of validation**: All validation happens at the op level
2. **Records aggregate op validity**: A record's validity is derived from its constituent ops
3. **Validation staging isolates pending data**: Unvalidated ops stay in staging until validated
4. **Distinct schemas per database type**: Each database has only the fields it needs
5. **Unified data storage**: DHT database serves both obligated and cached data, distinguished by arc coverage
6. **Clear state transitions**: Data moves through well-defined states with no ambiguity

### Database Structure

#### 1. Authored Database
**Purpose**: Store an agent's own authored chain data

```sql
-- Authored Actions (simplified, chain-focused)
CREATE TABLE Action (
    hash BLOB PRIMARY KEY,
    author BLOB NOT NULL,
    seq INTEGER NOT NULL,
    prev_hash BLOB,
    timestamp INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    
    -- Action-specific fields
    entry_hash BLOB,
    entry_type TEXT,
    -- ... other action-specific fields
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

#### 2. Validation Staging Database
**Purpose**: Hold ops during validation process

```sql
-- Staging area for ops being validated
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

#### 3. DHT Database
**Purpose**: Store validated DHT data

```sql
-- Validated Actions in DHT
CREATE TABLE DhtAction (
    hash BLOB PRIMARY KEY,
    author BLOB NOT NULL,
    timestamp INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    blob BLOB NOT NULL,
    
    -- Record validity (aggregated from ops)
    record_validity TEXT NOT NULL, -- 'valid', 'rejected', 'abandoned'
    
    -- Action-specific fields
    entry_hash BLOB,
    -- ... other fields
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
1. A record is **INVALID** if ANY of its ops are rejected
2. A record is **VALID** if ALL known ops are valid
3. A record is **PENDING** if some ops are pending validation
4. A record is **ABANDONED** if all ops are abandoned
5. A record's validity is computed on integration, not on query

The record validity is determined at the time of integration by examining all ops associated with the record. This aggregated status is stored with the record itself, eliminating the need for complex joins during queries.

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

1. **insert_op** (from network or author)
   - Insert into StagingOp with validation_stage='pending_sys'

2. **set_validation_status** (after validation)
   - Update StagingOp.sys/app_validation_status

3. **set_when_integrated** (after all validation)
   - Move from StagingOp to DhtOp
   - Update record validity in DhtAction/DhtEntry

4. **set_receipts_complete** (after enough receipts)
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
7. **Authored ops exist in both AuthoredOp and appropriate validation stage**
