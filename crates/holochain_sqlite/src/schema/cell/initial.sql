-- Initial Holochain Cell schema

CREATE TABLE Entry (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    -- might not need this index, let's avoid for now
    -- type             VARCHAR(64)    NOT NULL,

    blob             BLOB           NOT NULL,

    -- CapClaim / CapGrant
    tag              VARCHAR(64)           NULL,

    -- CapClaim
    grantor          BLOB           NULL,
    cap_secret       BLOB           NULL,

    -- CapGrant
    functions        BLOB           NULL,
    access_type      VARCHAR(64)    NULL,
    access_secret    BLOB           NULL,
    access_assignees BLOB           NULL
);
-- CREATE INDEX Entry_type_idx ON Entry ( type );


-- TODO: some of the NULL fields can be collapsed,
--       like between Update and Delete
CREATE TABLE Header (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    type             VARCHAR(64)    NOT NULL,
    seq              INTEGER        NOT NULL,
    author           BLOB           NOT NULL,

    blob             BLOB           NOT NULL,

    -- Create / Update
    entry_hash       BLOB           NULL,
    entry_type       VARCHAR(64)    NULL,  -- The opaque EntryType
    private_entry    INTEGER        NULL,  -- BOOLEAN

    -- Update
    original_entry_hash   BLOB      NULL,
    original_header_hash  BLOB      NULL,

    -- Delete
    deletes_entry_hash    BLOB      NULL,
    deletes_header_hash   BLOB      NULL,

    -- CreateLink
    -- NB: basis_hash can't be foreign key, since it could map to either
    --     Entry or Header
    -- FIXME: @freesig Actually this can only be an EntryHash.
    -- Links can't be on headers.
    base_hash        BLOB           NULL,
    zome_id          INTEGER        NULL,
    tag              BLOB           NULL,

    -- DeleteLink
    create_link_hash    BLOB           NULL,

    -- AgentValidationPkg
    membrane_proof   BLOB           NULL,

    -- OpenChain / CloseChain
    prev_dna_hash    BLOB           NULL

    -- We can't have any of these constraint because 
    -- the element authority doesn't get the create link for a remove link. @freesig
    -- FOREIGN KEY(entry_hash) REFERENCES Entry(hash)
    -- FOREIGN KEY(original_entry_hash) REFERENCES Entry(hash),
    -- FOREIGN KEY(original_header_hash) REFERENCES Header(hash),
    -- FOREIGN KEY(deletes_entry_hash) REFERENCES Entry(hash)
    -- FOREIGN KEY(deletes_header_hash) REFERENCES Header(hash),
    -- FOREIGN KEY(create_link_hash) REFERENCES Header(hash)
);
CREATE INDEX Header_type_idx ON Header ( type );
CREATE INDEX Header_author ON Header ( author );


-- NB: basis_hash, header_hash, and entry_hash, in general, will have
--     duplication of data. Could rethink these a bit.
CREATE TABLE DhtOp (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    type             VARCHAR(64)    NOT NULL,
    basis_hash       BLOB           NOT NULL,
    header_hash      BLOB           NOT NULL,
    is_authored      INTEGER        NOT NULL,      -- BOOLEAN
    require_receipt  INTEGER        NOT NULL,      -- BOOLEAN

    -- This is the order that process ops should result 
    -- in dependencies before dependants.
    -- See OpOrder.
    op_order        TEXT           NOT NULL,

    -- If this is null then validation is still in progress.
    validation_status INTEGER       NULL,

    when_integrated  INTEGER NULL,          -- DATETIME
    -- We need nanosecond accuracy which doesn't fit in 
    -- an INTEGER.
    when_integrated_ns  BLOB NULL,          -- DATETIME

    receipt_count       INTEGER     NULL,
    last_publish_time   INTEGER     NULL,   -- UNIX TIMESTAMP SECONDS

    blob             BLOB           NOT NULL,

    -- 0: Awaiting System Validation Dependencies.
    -- 1: Successfully System Validated (And ready for app validation).
    -- 2: Awaiting App Validation Dependencies.
    -- 3: Awaiting integration.
    -- Don't need the other stages (pending, awaiting itntegration) because:
    -- - pending = validation_stage null && validation_status null.
    -- We could make this an enum and use a Blob so we can capture which 
    -- deps are being awaited for debugging.
    validation_stage    INTEGER     NULL,

    -- NB: I removed this because when_integrated covers it
    -- TODO: @freesig: Might be hard to index on various timestamps?
    -- is_integrated    INTEGER        NOT NULL,      -- BOOLEAN

    -- NB: I removed this because it's accessible via Header.entry_hash
    -- entry_hash       BLOB           NULL,

    FOREIGN KEY(header_hash) REFERENCES Header(hash)
);
CREATE INDEX DhtOp_type_idx ON DhtOp ( type );
-- CREATE INDEX DhtOp_basis_hash_idx ON DhtOp ( basis_hash );
