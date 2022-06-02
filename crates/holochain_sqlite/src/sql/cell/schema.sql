-- no-sql-format --

-- Initial Holochain Cell schema

CREATE TABLE IF NOT EXISTS Entry (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    -- might not need this index, let's avoid for now
    -- type             VARCHAR(64)    NOT NULL,

    blob             BLOB           NOT NULL,

    -- CapClaim / CapGrant
    tag              TEXT           NULL,

    -- CapClaim
    grantor          BLOB           NULL,
    cap_secret       BLOB           NULL,

    -- CapGrant
    functions        BLOB           NULL,
    access_type      TEXT           NULL,
    access_secret    BLOB           NULL,
    access_assignees BLOB           NULL
);
-- CREATE INDEX Entry_type_idx ON Entry ( type );


-- TODO: some of the NULL fields can be collapsed,
--       like between Update and Delete
CREATE TABLE IF NOT EXISTS Action (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    type             TEXT           NOT NULL,
    seq              INTEGER        NOT NULL,
    author           BLOB           NOT NULL,

    blob             BLOB           NOT NULL,
    prev_hash        BLOB           NULL,

    -- Create / Update
    entry_hash       BLOB           NULL,
    entry_type       TEXT           NULL,  -- The opaque EntryType
    private_entry    INTEGER        NULL,  -- BOOLEAN

    -- Update
    original_entry_hash   BLOB      NULL,
    original_action_hash  BLOB      NULL,

    -- Delete
    deletes_entry_hash    BLOB      NULL,
    deletes_action_hash   BLOB      NULL,

    -- CreateLink
    -- NB: basis_hash can't be foreign key, since it could map to either
    --     Entry or Action
    base_hash        BLOB           NULL,
    zome_id          INTEGER        NULL,
    link_type        INTEGER        NULL,
    tag              BLOB           NULL,

    -- DeleteLink
    create_link_hash    BLOB           NULL,

    -- AgentValidationPkg
    membrane_proof   BLOB           NULL,

    -- OpenChain / CloseChain
    prev_dna_hash    BLOB           NULL,

    -- Rate limiting intermediate state
    rate_bucket_id        INTEGER        NOT NULL,
    rate_bucket_state     BLOB           NOT NULL

    -- We can't have any of these constraint because
    -- the record authority doesn't get the create link for a remove link. @freesig
    -- FOREIGN KEY(entry_hash) REFERENCES Entry(hash)
    -- FOREIGN KEY(original_entry_hash) REFERENCES Entry(hash),
    -- FOREIGN KEY(original_action_hash) REFERENCES Action(hash),
    -- FOREIGN KEY(deletes_entry_hash) REFERENCES Entry(hash)
    -- FOREIGN KEY(deletes_action_hash) REFERENCES Action(hash),
    -- FOREIGN KEY(create_link_hash) REFERENCES Action(hash)
);
CREATE INDEX IF NOT EXISTS Action_type_idx ON Action ( type );
CREATE INDEX IF NOT EXISTS Action_author ON Action ( author );
CREATE INDEX IF NOT EXISTS Action_seq_idx ON Action ( seq );


-- NB: basis_hash, action_hash, and entry_hash, in general, will have
--     duplication of data. Could rethink these a bit.
CREATE TABLE IF NOT EXISTS DhtOp (
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
    -- Don't need the other stages (pending, awaiting itntegration) because:
    -- - pending = validation_stage null && validation_status null.
    -- We could make this an enum and use a Blob so we can capture which
    -- deps are being awaited for debugging.
    validation_stage            INTEGER     NULL,
    num_validation_attempts     INTEGER     NULL,
    last_validation_attempt     INTEGER     NULL,

    -- NB: I removed this because when_integrated covers it
    -- TODO: @freesig: Might be hard to index on various timestamps?
    -- is_integrated    INTEGER        NOT NULL,      -- BOOLEAN

    -- NB: I removed this because it's accessible via Action.entry_hash
    -- entry_hash       BLOB           NULL,

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

CREATE TABLE IF NOT EXISTS ValidationReceipt (
    hash            BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    op_hash         BLOB           NOT NULL,
    blob            BLOB           NOT NULL,
    FOREIGN KEY(op_hash) REFERENCES DhtOp(hash)
);

CREATE TABLE IF NOT EXISTS ChainLock (
    lock BLOB PRIMARY KEY ON CONFLICT ROLLBACK,
    author BLOB NOT NULL,
    -- The expiration time of the lock as a Timestamp (microseconds)
    expires_at_timestamp INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ScheduledFunctions (
    author BLOB NOT NULL,
    zome_name TEXT NOT NULL,
    scheduled_fn TEXT NOT NULL,
    maybe_schedule BLOB NOT NULL,
    start INTEGER NOT NULL,
    end INTEGER NOT NULL,
    ephemeral BOOLEAN NOT NULL,
    PRIMARY KEY (zome_name, scheduled_fn, author) ON CONFLICT ROLLBACK
);

