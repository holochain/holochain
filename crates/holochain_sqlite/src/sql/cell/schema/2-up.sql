-- no-sql-format --

DROP INDEX Action_type_idx;
DROP INDEX Action_author;
DROP INDEX Action_seq_idx;

CREATE TABLE Action_2up (
    hash             BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    type             TEXT           NOT NULL,
    author           BLOB           NOT NULL,

    blob             BLOB           NOT NULL,
    prev_hash        BLOB           NULL,

    -- Actions only
    seq              INTEGER        NULL,

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
    zome_index       INTEGER        NULL,
    link_type        INTEGER        NULL,
    tag              BLOB           NULL,

    -- DeleteLink
    create_link_hash    BLOB           NULL,

    -- AgentValidationPkg
    membrane_proof   BLOB           NULL,

    -- OpenChain / CloseChain
    prev_dna_hash    BLOB           NULL

    -- We can't have any of these constraint because
    -- the record authority doesn't get the create link for a remove link. @freesig
    -- FOREIGN KEY(entry_hash) REFERENCES Entry(hash)
    -- FOREIGN KEY(original_entry_hash) REFERENCES Entry(hash),
    -- FOREIGN KEY(original_action_hash) REFERENCES Action(hash),
    -- FOREIGN KEY(deletes_entry_hash) REFERENCES Entry(hash)
    -- FOREIGN KEY(deletes_action_hash) REFERENCES Action(hash),
    -- FOREIGN KEY(create_link_hash) REFERENCES Action(hash)
);

INSERT INTO Action_2up SELECT * FROM Action;

DROP TABLE Action;

ALTER TABLE Action_2up RENAME TO Action;

CREATE INDEX Action_type_idx ON Action ( type );
CREATE INDEX Action_author ON Action ( author );
CREATE INDEX Action_seq_idx ON Action ( seq );



------------------------------------------------------------------------

ALTER TABLE DhtOp 
    ADD COLUMN  dependency2  BLOB  NULL;

DROP INDEX DhtOp_type_dep_idx;
DROP INDEX DhtOp_validation_stage_idx;

CREATE INDEX DhtOp_type_dep_idx ON DhtOp ( type, dependency, dependency2 );
CREATE INDEX DhtOp_validation_stage_idx ON DhtOp ( validation_stage, type, dependency, dependency2 );
