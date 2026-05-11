-- DHT database schema (per-DNA). See docs/design/state_model.md.
--
-- Integer convention summary:
--   ActionType         : 1..=10 (Dna, AgentValidationPkg, InitZomesComplete,
--                                Create, Update, Delete, CreateLink, DeleteLink,
--                                CloseChain, OpenChain)
--   ChainOpType        : 1..=9 (see state_model.md)
--   CapAccess          : 0=Unrestricted, 1=Transferable, 2=Assigned
--   record_validity /
--   sys_validation_status /
--   app_validation_status: NULL=pending, 1=accepted, 2=rejected
--   Booleans           : stored as INTEGER 0/1
--
-- Foreign-key delete behaviour:
--   Index tables (Link, DeletedLink, UpdatedRecord, DeletedRecord) cascade on
--   delete of the referenced Action — the index is derivative and must follow
--   the parent. All other FKs (CapGrant, LimboChainOp, ChainOp, ChainOpPublish,
--   ValidationReceipt, WarrantPublish) intentionally do NOT cascade: deletes
--   must be done explicitly by workflow code, so accidental loss of first-class
--   state can't happen via parent removal.

-- Actions: both self-authored and network-received.
CREATE TABLE Action (
    hash            BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    author          BLOB    NOT NULL,
    seq             INTEGER NOT NULL,
    prev_hash       BLOB,                     -- NULL only for the genesis Dna action
    timestamp       INTEGER NOT NULL,
    action_type     INTEGER NOT NULL,
    action_data     BLOB    NOT NULL,         -- serialized ActionData
    signature       BLOB    NOT NULL,

    -- No FK to Entry: private entries live in PrivateEntry, and public
    -- entries may not yet have arrived when the action is inserted.
    entry_hash      BLOB,
    private_entry   INTEGER,                  -- 0/1, NULL when no entry

    record_validity INTEGER                   -- NULL=pending, 1=accepted, 2=rejected
) STRICT, WITHOUT ROWID;

-- Public entries.
CREATE TABLE Entry (
    hash BLOB PRIMARY KEY ON CONFLICT IGNORE,
    blob BLOB NOT NULL
) STRICT, WITHOUT ROWID;

-- Private entries (local author only).
CREATE TABLE PrivateEntry (
    hash   BLOB PRIMARY KEY ON CONFLICT IGNORE,
    author BLOB NOT NULL,
    blob   BLOB NOT NULL
) STRICT, WITHOUT ROWID;

-- Capability grants index.
CREATE TABLE CapGrant (
    action_hash BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    cap_access  INTEGER NOT NULL,             -- 0=Unrestricted, 1=Transferable, 2=Assigned
    tag         TEXT,
    FOREIGN KEY(action_hash) REFERENCES Action(hash)
) STRICT, WITHOUT ROWID;

-- Capability claims (not chain entries).
CREATE TABLE CapClaim (
    id      INTEGER PRIMARY KEY,              -- rowid alias
    author  BLOB NOT NULL,
    tag     TEXT NOT NULL,
    grantor BLOB NOT NULL,
    secret  BLOB NOT NULL
) STRICT;

-- Chain lock (one per author).
CREATE TABLE ChainLock (
    author               BLOB    PRIMARY KEY,
    subject              BLOB    NOT NULL,
    expires_at_timestamp INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Limbo for network-received chain ops awaiting validation.
CREATE TABLE LimboChainOp (
    hash                    BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    op_type                 INTEGER NOT NULL,
    action_hash             BLOB    NOT NULL,

    basis_hash              BLOB    NOT NULL,
    storage_center_loc      INTEGER NOT NULL,

    sys_validation_status   INTEGER,
    app_validation_status   INTEGER,
    abandoned_at            INTEGER,

    require_receipt         INTEGER NOT NULL,
    when_received           INTEGER NOT NULL,
    sys_validation_attempts INTEGER NOT NULL DEFAULT 0,
    app_validation_attempts INTEGER NOT NULL DEFAULT 0,
    last_validation_attempt INTEGER,

    serialized_size         INTEGER NOT NULL,

    FOREIGN KEY(action_hash) REFERENCES Action(hash)
) STRICT, WITHOUT ROWID;

-- Limbo for network-received warrants awaiting validation.
CREATE TABLE LimboWarrant (
    hash                    BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    author                  BLOB    NOT NULL,
    timestamp               INTEGER NOT NULL,
    warrantee               BLOB    NOT NULL,
    proof                   BLOB    NOT NULL,

    storage_center_loc      INTEGER NOT NULL,

    sys_validation_status   INTEGER,
    abandoned_at            INTEGER,

    when_received           INTEGER NOT NULL,
    sys_validation_attempts INTEGER NOT NULL DEFAULT 0,
    last_validation_attempt INTEGER,

    serialized_size         INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Integrated chain ops.
CREATE TABLE ChainOp (
    hash               BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    op_type            INTEGER NOT NULL,
    action_hash        BLOB    NOT NULL,

    basis_hash         BLOB    NOT NULL,
    storage_center_loc INTEGER NOT NULL,

    validation_status  INTEGER NOT NULL,
    locally_validated  INTEGER NOT NULL,

    when_received      INTEGER NOT NULL,
    when_integrated    INTEGER NOT NULL,

    serialized_size    INTEGER NOT NULL,

    FOREIGN KEY(action_hash) REFERENCES Action(hash)
) STRICT, WITHOUT ROWID;

-- Publish state for self-authored chain ops.
CREATE TABLE ChainOpPublish (
    op_hash           BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    last_publish_time INTEGER,
    receipts_complete INTEGER,
    withhold_publish  INTEGER,
    FOREIGN KEY(op_hash) REFERENCES ChainOp(hash)
) STRICT, WITHOUT ROWID;

-- Validation receipts for authored ops.
CREATE TABLE ValidationReceipt (
    hash          BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    op_hash       BLOB    NOT NULL,
    validators    BLOB    NOT NULL,
    signature     BLOB    NOT NULL,
    when_received INTEGER NOT NULL,
    FOREIGN KEY(op_hash) REFERENCES ChainOp(hash)
) STRICT, WITHOUT ROWID;

-- Integrated warrants.
CREATE TABLE Warrant (
    hash               BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    author             BLOB    NOT NULL,
    timestamp          INTEGER NOT NULL,
    warrantee          BLOB    NOT NULL,
    proof              BLOB    NOT NULL,
    storage_center_loc INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Publish state for self-authored warrants.
CREATE TABLE WarrantPublish (
    warrant_hash      BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    last_publish_time INTEGER,
    FOREIGN KEY(warrant_hash) REFERENCES Warrant(hash)
) STRICT, WITHOUT ROWID;

-- Link index.
CREATE TABLE Link (
    action_hash BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    base_hash   BLOB    NOT NULL,
    zome_index  INTEGER NOT NULL,
    link_type   INTEGER NOT NULL,
    tag         BLOB,
    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

-- Deleted-link index.
CREATE TABLE DeletedLink (
    action_hash      BLOB PRIMARY KEY ON CONFLICT IGNORE,
    create_link_hash BLOB NOT NULL,
    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

-- Updated-record index.
CREATE TABLE UpdatedRecord (
    action_hash          BLOB PRIMARY KEY ON CONFLICT IGNORE,
    original_action_hash BLOB NOT NULL,
    original_entry_hash  BLOB NOT NULL,
    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

-- Deleted-record index.
CREATE TABLE DeletedRecord (
    action_hash         BLOB PRIMARY KEY ON CONFLICT IGNORE,
    deletes_action_hash BLOB NOT NULL,
    deletes_entry_hash  BLOB NOT NULL,
    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
) STRICT, WITHOUT ROWID;

-- Scheduled function records (per-author within this DNA's DB).
CREATE TABLE ScheduledFunction (
    author         BLOB    NOT NULL,
    zome_name      TEXT    NOT NULL,
    scheduled_fn   TEXT    NOT NULL,
    maybe_schedule BLOB    NOT NULL,
    start_at       INTEGER NOT NULL,
    end_at         INTEGER NOT NULL,
    ephemeral      INTEGER NOT NULL,             -- 0/1
    PRIMARY KEY (author, zome_name, scheduled_fn) ON CONFLICT ROLLBACK
) STRICT, WITHOUT ROWID;
