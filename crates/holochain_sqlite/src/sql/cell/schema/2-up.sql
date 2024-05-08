-- no-sql-format --

DROP INDEX DhtOp_type_dep_idx;
DROP INDEX DhtOp_type_when_int_idx;
DROP INDEX DhtOp_validation_stage_idx;
DROP INDEX DhtOp_stage_type_status_idx;
DROP INDEX DhtOp_validation_status_idx;
DROP INDEX DhtOp_authored_timestamp_idx;
DROP INDEX DhtOp_storage_center_loc_idx;
DROP INDEX DhtOp_action_hash_idx;
DROP INDEX DhtOp_basis_hash_idx;


-- create new table
CREATE TABLE DhtOp_2up (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    type             TEXT           NOT NULL,
    basis_hash       BLOB           NOT NULL,
    action_hash      BLOB           NOT NULL,
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

INSERT INTO DhtOp_2up SELECT * FROM DhtOp;

DROP TABLE DhtOp;

ALTER TABLE DhtOp_2up RENAME TO DhtOp;

CREATE INDEX DhtOp_type_dep_idx ON DhtOp ( type, dependency );
CREATE INDEX DhtOp_type_when_int_idx ON DhtOp ( type, when_integrated );
CREATE INDEX DhtOp_validation_stage_idx ON DhtOp ( validation_stage, type, dependency );
CREATE INDEX DhtOp_stage_type_status_idx ON DhtOp ( validation_stage, type, validation_status);
CREATE INDEX DhtOp_validation_status_idx ON DhtOp ( validation_status );
CREATE INDEX DhtOp_authored_timestamp_idx ON DhtOp ( authored_timestamp );
CREATE INDEX DhtOp_storage_center_loc_idx ON DhtOp ( storage_center_loc );
CREATE INDEX DhtOp_action_hash_idx ON DhtOp ( action_hash );
CREATE INDEX DhtOp_basis_hash_idx ON DhtOp ( basis_hash );


CREATE TABLE Warrant (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,

    -- it is best if these two fields match the analogous fields 
    -- in the Action table
    type             TEXT           NOT NULL,
    author           BLOB           NOT NULL,

    -- the offending thing that the warrant is for,
    -- e.g. an ActionHash or an AgentPubKey
    target           BLOB           NOT NULL,
);