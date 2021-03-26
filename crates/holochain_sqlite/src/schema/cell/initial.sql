-- Initial Holochain Cell schema
CREATE TABLE IF NOT EXISTS Entry (
    hash BLOB PRIMARY KEY,
    type VARCHAR(64) NOT NULL,
    blob BLOB NOT NULL,
    -- CapClaim / CapGrant
    tag VARCHAR(64) NULL,
    -- CapClaim
    grantor BLOB NULL,
    cap_secret BLOB NULL,
    -- CapGrant
    functions BLOB NULL,
    access_type VARCHAR(64) NULL,
    access_secret BLOB NULL,
    access_assignees BLOB NULL
);
CREATE INDEX Entry_type_idx ON Entry (type);
-- TODO: some of the NULL fields can be collapsed,
--       like between Update and Delete
CREATE TABLE IF NOT EXISTS Header (
    hash BLOB PRIMARY KEY,
    type VARCHAR(64) NOT NULL,
    seq INTEGER NOT NULL,
    blob BLOB NOT NULL,
    -- Create / Update
    entry_hash BLOB NULL,
    entry_type VARCHAR(64) NULL,
    -- Update
    original_entry_hash BLOB NULL,
    original_header_hash BLOB NULL,
    -- Delete
    deletes_entry_hash BLOB NULL,
    deletes_header_hash BLOB NULL,
    -- CreateLink
    -- NB: basis_hash can't be foreign key, since it could map to either
    --     Entry or Header
    basis_hash BLOB NULL,
    zome_id INTEGER NULL,
    tag BLOB NULL,
    -- DeleteLink
    link_add_hash BLOB NULL,
    -- AgentValidationPkg
    membrane_proof BLOB NULL,
    -- OpenChain / CloseChain
    prev_dna_hash BLOB NULL,
    FOREIGN KEY(entry_hash) REFERENCES Entry(hash),
    FOREIGN KEY(original_entry_hash) REFERENCES Entry(hash),
    -- FOREIGN KEY(original_header_hash) REFERENCES Header(hash),
    FOREIGN KEY(deletes_entry_hash) REFERENCES Entry(hash),
    -- FOREIGN KEY(deletes_header_hash) REFERENCES Header(hash),
    FOREIGN KEY(link_add_hash) REFERENCES Header(hash)
);
CREATE INDEX Header_type_idx ON Header (type);
-- NB: basis_hash, header_hash, and entry_hash, in general, will have
--     duplication of data. Could rethink these a bit.
CREATE TABLE IF NOT EXISTS DhtOp (
    hash BLOB PRIMARY KEY,
    type VARCHAR(64) NOT NULL,
    basis_hash BLOB NOT NULL,
    header_hash BLOB NOT NULL,
    entry_hash BLOB NULL,
    -- TODO: necessary?
    is_authored INTEGER NOT NULL,
    -- BOOLEAN
    is_integrated INTEGER NOT NULL,
    -- BOOLEAN
    require_receipt INTEGER NOT NULL,
    -- BOOLEAN
    when_integrated NUMERIC NULL,
    -- DATETIME
    blob BLOB NOT NULL,
    FOREIGN KEY(header_hash) REFERENCES Header(hash),
    FOREIGN KEY(entry_hash) REFERENCES Entry(hash)
);
CREATE INDEX DhtOp_type_idx ON DhtOp (type);