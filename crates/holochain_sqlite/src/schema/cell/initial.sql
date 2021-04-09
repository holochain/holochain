-- Initial Holochain Cell schema

CREATE TABLE Entry (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    type             VARCHAR(64)    NOT NULL,

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
CREATE INDEX Entry_type_idx ON Entry ( type );


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
    entry_type       VARCHAR(64)    NULL,

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
    prev_dna_hash    BLOB           NULL,

    FOREIGN KEY(entry_hash) REFERENCES Entry(hash)
    -- We can't have any of these constraint because 
    -- the element authority doesn't get the create link for a remove link. @freesig
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

    -- If this is null then validation is still in progress.
    validation_status INTEGER       NULL,

    when_integrated  NUMERIC        NULL,          -- DATETIME

    blob             BLOB           NOT NULL,

    -- NB: I removed this because when_integrated covers it
    -- TODO: @freesig: Might be hard to index on various timestamps?
    -- is_integrated    INTEGER        NOT NULL,      -- BOOLEAN

    -- NB: I removed this because it's accessible via Header.entry_hash
    -- entry_hash       BLOB           NULL,

    FOREIGN KEY(header_hash) REFERENCES Header(hash)
);
CREATE INDEX DhtOp_type_idx ON DhtOp ( type );
