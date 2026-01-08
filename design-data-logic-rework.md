# Data Logic Rework Design Document

## Executive Summary

This document outlines a major refactoring of the Holochain data storage and validation logic to address fundamental issues with the current implementation. The key goals are:

1. Establish ops as the unit of validation with aggregated validity status for records
2. Introduce a validation staging area to separate pending data from validated data
3. Eliminate the separate cache database (redundant with DHT + arc tracking)
4. Create distinct schemas for authored, DHT, and validation staging databases
5. Simplify and correct data queries throughout the system

## Current Problems

### Mixed Validation Model
- **Issue**: Validation status is tracked on individual ops but queries join on DhtOp to determine record validity
- **Impact**: Endless inconsistencies where records appear valid/invalid based on which op type is queried
- **Example**: A record with multiple ops (StoreRecord, StoreEntry, RegisterAgentActivity) may have different validation status for each, leading to inconsistent results

### Schema Confusion
- **Issue**: Single schema shared between authored, DHT, and cache databases despite different requirements
- **Impact**: Many unused fields in each database context, leading to confusion and bugs
- **Example**: `validation_status`, `when_integrated` fields are meaningless in authored DB but present anyway

### No Validation Staging
- **Issue**: Pending validation data sits in main DHT tables requiring complex filtering
- **Impact**: Performance degradation, complex queries, risk of serving unvalidated data
- **Example**: Every DHT query must filter on validation_status IS NOT NULL

### Redundant Cache Database
- **Issue**: Separate cache database duplicates DHT data
- **Impact**: Unnecessary storage overhead and synchronization complexity
- **Solution**: Track cached content via agent arc coverage instead

## New Architecture

### Core Principles

1. **Ops are the unit of validation**: All validation happens at the op level
2. **Records aggregate op validity**: A record's validity is derived from its constituent ops
3. **Validation staging isolates pending data**: Unvalidated ops stay in staging until validated
4. **Distinct schemas per database type**: Each database has only the fields it needs
5. **No separate cache**: Cached content determined by arc coverage
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

**Implementation:**
```sql
-- When integrating an op, update record validity
UPDATE DhtAction 
SET record_validity = CASE
    WHEN EXISTS (SELECT 1 FROM DhtOp WHERE action_hash = ? AND validation_status = 'rejected') 
        THEN 'rejected'
    WHEN EXISTS (SELECT 1 FROM StagingOp WHERE action_hash = ?)
        THEN 'pending'
    WHEN NOT EXISTS (SELECT 1 FROM DhtOp WHERE action_hash = ? AND validation_status != 'valid')
        THEN 'valid'
    ELSE 'unknown'
END
WHERE hash = ?;
```

## Migration Path

### Phase 1: Create New Schema
1. Create validation staging database alongside existing databases
2. Create new DHT tables with proper structure
3. Implement data migration scripts

### Phase 2: Update Workflows
1. Modify validation workflows to use staging database
2. Update integration workflow to move data from staging to DHT
3. Implement record validity aggregation

### Phase 3: Update Queries
1. Rewrite all record queries to use DhtAction/DhtEntry directly
2. Remove joins on DhtOp for validity checks
3. Update cascade to query from appropriate database

### Phase 4: Cleanup
1. Remove cache database
2. Remove old DHT tables
3. Clean up unused code paths

## Query Transformations

### Example: Get Record by Hash

**Current (problematic):**
```sql
SELECT Action.*, Entry.*, DhtOp.validation_status
FROM Action
LEFT JOIN Entry ON Action.entry_hash = Entry.hash  
INNER JOIN DhtOp ON DhtOp.action_hash = Action.hash
WHERE Action.hash = ? 
  AND DhtOp.type = 'StoreRecord'
  AND DhtOp.validation_status IS NOT NULL
```

**New (simplified):**
```sql
SELECT Action.*, Entry.*
FROM DhtAction AS Action
LEFT JOIN DhtEntry AS Entry ON Action.entry_hash = Entry.hash
WHERE Action.hash = ?
  AND Action.record_validity = 'valid'
```

### Example: Get Links

**Current:**
```sql
SELECT Action.*, DhtOp.validation_status
FROM Action
INNER JOIN DhtOp ON DhtOp.action_hash = Action.hash  
WHERE DhtOp.type = 'RegisterAddLink'
  AND DhtOp.basis_hash = ?
  AND DhtOp.validation_status = 0
```

**New:**
```sql
SELECT *
FROM DhtLink
WHERE base_hash = ?
  AND is_deleted = FALSE
```

## Benefits

1. **Correctness**: Record validity properly aggregated from all ops
2. **Performance**: No complex joins for basic queries
3. **Clarity**: Each database has clear purpose and schema
4. **Safety**: Unvalidated data isolated in staging
5. **Simplicity**: No redundant cache database
6. **Maintainability**: Cleaner separation of concerns

## Implementation Considerations

### Performance
- Index strategy for new tables needs careful planning
- Migration of existing data must be batched
- Staging database should use WAL mode for concurrent access

### Backwards Compatibility
- Need compatibility layer during migration
- Existing hApp code should continue working
- Network protocol changes may be needed

### Testing
- Comprehensive test suite for data migration
- Validation of record aggregation logic
- Performance benchmarks before/after

## Open Questions

1. Should we keep historical validation attempts in staging or purge after integration?
2. How to handle partial record visibility during validation?
3. Should rejected ops be kept indefinitely or pruned?
4. Network protocol changes needed for validation receipts?
5. How to handle warrant ops in the new model?

## Next Steps

1. Review and refine this design with team
2. Create detailed migration plan
3. Implement proof-of-concept for staging database
4. Benchmark performance implications
5. Plan phased rollout strategy

## Appendix A: Workflow Changes

### Incoming DHT Ops Workflow
**Current**: Ops inserted directly into main DHT database with NULL validation_status
**New**: Ops inserted into StagingOp table with validation_stage='pending_sys'

### Sys Validation Workflow  
**Current**: Updates validation_status in DhtOp table
**New**: Updates sys_validation_status in StagingOp, triggers app validation or abandonment

### App Validation Workflow
**Current**: Updates validation_status and when_app_validated in DhtOp
**New**: Updates app_validation_status in StagingOp, triggers integration if valid

### Integration Workflow
**Current**: Sets when_integrated timestamp on DhtOp
**New**: Moves validated ops from StagingOp to DhtOp, updates record validity in DhtAction/DhtEntry

### Publish DHT Ops Workflow
**Current**: Queries DhtOp for ops to publish based on when_integrated
**New**: Queries DhtOp for ops to publish (all ops in DhtOp are integrated)

### Validation Receipt Workflow
**Current**: Inserts receipts linked to DhtOp
**New**: Inserts receipts linked to StagingOp, updates receipts_complete when moved to DhtOp

## Appendix B: Arc and Caching Strategy

Without a separate cache database, the system needs to track what data is "cached" (held for others) versus what is "authored" (our own data). This is accomplished through:

1. **Arc Coverage Tracking**: Each node tracks its coverage of the DHT address space
2. **Storage Obligations**: Data within our arc is obligated storage
3. **Cached Data**: Data outside our arc but held temporarily

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

-- In DhtOp table, use storage_center_loc to determine if within arc
-- If within arc: obligated storage
-- If outside arc: cached (can be pruned)
```

### Cache Pruning Strategy

When storage pressure occurs:
1. Identify ops outside current arc (storage_center_loc not in [arc_start, arc_end])
2. Sort by last_access_time (new field needed)
3. Prune oldest first until storage target met
4. Never prune authored ops or ops within arc

## Appendix C: Warrant Handling

Warrants require special consideration in the new model:

1. **ChainIntegrityWarrant**: Proves an author broke chain rules
   - Stays in StagingOp until warranted action is fetched and validated
   - If warranted action is rejected, warrant moves to DhtOp as valid
   - If warranted action is valid, warrant is rejected

2. **Warrant Validation Dependencies**: 
   - Warrants can have up to 2 dependencies (the two actions being warranted)
   - These must be resolved before warrant validation can complete

```sql
-- In StagingOp, warrants use dependency1 and dependency2 fields
-- to track the actions they're warranting
```

## Appendix D: Critical Mutations and Queries

### Key Mutations

1. **insert_op** (from network or author)
   - Old: Insert into DhtOp with validation_status NULL
   - New: Insert into StagingOp with validation_stage='pending_sys'

2. **set_validation_status** (after validation)
   - Old: Update DhtOp.validation_status
   - New: Update StagingOp.sys/app_validation_status

3. **set_when_integrated** (after all validation)
   - Old: Update DhtOp.when_integrated timestamp
   - New: Move from StagingOp to DhtOp, update record validity

4. **set_receipts_complete** (after enough receipts)
   - Old: Update DhtOp.receipts_complete
   - New: Update DhtOp.receipts_complete (same, but only for integrated ops)

### Key Queries

1. **get_record** (by action hash)
   - Old: Complex join of Action + Entry + DhtOp filtered by validation
   - New: Simple query of DhtAction + DhtEntry by record_validity

2. **get_entry** (by entry hash)  
   - Old: Join Entry + Action + DhtOp, check multiple op types
   - New: Direct query of DhtEntry by validity

3. **get_links** (by base hash)
   - Old: Join Action + DhtOp filtered by RegisterAddLink ops
   - New: Direct query of DhtLink table

4. **agent_activity** (by agent)
   - Old: Complex query with multiple validation checks
   - New: Query DhtAction by author and record_validity

5. **validation_limbo** (ops awaiting validation)
   - Old: Query DhtOp where validation_status IS NULL
   - New: Query StagingOp by validation_stage

## Appendix E: Data Integrity Invariants

The new system must maintain these invariants:

1. **No op exists in both StagingOp and DhtOp simultaneously**
2. **Every DhtOp has a definite validation_status (never NULL)**
3. **Every DhtAction has a computed record_validity**
4. **Ops move from staging to DHT atomically with record updates**
5. **Rejected ops in DhtOp cause their records to be marked invalid**
6. **Dependencies are resolved before validation proceeds**
7. **Authored ops exist in both AuthoredOp and appropriate validation stage**
