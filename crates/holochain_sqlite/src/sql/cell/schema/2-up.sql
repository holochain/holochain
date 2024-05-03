-- no-sql-format --

PRAGMA foreign_keys = off;

-- backup existing table
ALTER TABLE
  DhtOp RENAME TO DhtOp_old;

-- create new table
CREATE TABLE DhtOp (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    type             TEXT           NOT NULL,
    basis_hash       BLOB           NOT NULL,
    action_hash      BLOB           NULL,
    require_receipt  INTEGER        NOT NULL,      -- BOOLEAN

    storage_center_loc          INTEGER   NOT NULL,
    authored_timestamp       INTEGER   NOT NULL,

    -- This is the order that process ops should result
    -- in dependencies before dependants.
    -- See OpOrder.
    op_order        TEXT           NOT NULL,

    -- If this is null then validation is still in progress.
    validation_status INTEGER       NULL,

    when_integrated   INTEGER       NULL,          -- DATETIME

    -- Used to withhold ops from publishing for things
    -- like countersigning.
    withhold_publish    INTEGER     NULL, -- BOOLEAN

    -- The op has received enough validation receipts.
    -- This is required as a field because different ops have different EntryTypes,
    -- which have different numbers of required validation receipts.
    receipts_complete   INTEGER     NULL,     -- BOOLEAN

    last_publish_time   INTEGER     NULL,   -- UNIX TIMESTAMP SECONDS

    -- 0: Awaiting System Validation Dependencies.
    -- 1: Successfully System Validated (And ready for app validation).
    -- 2: Awaiting App Validation Dependencies.
    -- 3: Awaiting integration.
    -- Don't need the other stages (pending, awaiting integration) because:
    -- - pending = validation_stage null && validation_status null.
    -- We could make this an enum and use a Blob so we can capture which
    -- deps are being awaited for debugging.
    validation_stage            INTEGER     NULL,
    num_validation_attempts     INTEGER     NULL,
    last_validation_attempt     INTEGER     NULL,

    -- The integration dependency if there is one.
    dependency          BLOB           NULL,

    FOREIGN KEY(action_hash) REFERENCES Action(hash) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS DhtOp_type_dep_idx ON DhtOp ( type, dependency );
CREATE INDEX IF NOT EXISTS DhtOp_type_when_int_idx ON DhtOp ( type, when_integrated );
CREATE INDEX IF NOT EXISTS DhtOp_validation_stage_idx ON DhtOp ( validation_stage, type, dependency );
CREATE INDEX IF NOT EXISTS DhtOp_stage_type_status_idx ON DhtOp ( validation_stage, type, validation_status);
CREATE INDEX IF NOT EXISTS DhtOp_validation_status_idx ON DhtOp ( validation_status );
CREATE INDEX IF NOT EXISTS DhtOp_authored_timestamp_idx ON DhtOp ( authored_timestamp );
CREATE INDEX IF NOT EXISTS DhtOp_storage_center_loc_idx ON DhtOp ( storage_center_loc );
CREATE INDEX IF NOT EXISTS DhtOp_action_hash_idx ON DhtOp ( action_hash );
CREATE INDEX IF NOT EXISTS DhtOp_basis_hash_idx ON DhtOp ( basis_hash );

-- copy data
INSERT INTO DhtOp SELECT * FROM DhtOp_old;

-- cleanup
PRAGMA foreign_keys = on;