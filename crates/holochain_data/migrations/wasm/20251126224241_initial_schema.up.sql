-- Wasm database schema for Holochain

-- Wasm bytecode storage
CREATE TABLE IF NOT EXISTS Wasm (
    hash            BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    code            BLOB           NOT NULL
);

-- Compiled, serialized WASM module storage
CREATE TABLE IF NOT EXISTS CompiledWasm (
    hash            BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    serialized        BLOB           NOT NULL,

    FOREIGN KEY (hash) REFERENCES Wasm (hash) ON DELETE CASCADE
);

-- DnaDef storage (flattened from DnaDef struct)
CREATE TABLE IF NOT EXISTS DnaDef (
    hash            BLOB           NOT NULL,
    agent           BLOB           NOT NULL,  -- Agent public key (32 bytes)
    name            TEXT           NOT NULL,
    network_seed    TEXT           NOT NULL,
    properties      BLOB           NOT NULL,  -- SerializedBytes
    lineage         JSON,                     -- JSON HashSet<DnaHash>
    PRIMARY KEY (hash, agent)
);

-- IntegrityZome storage (one row per zome in a DNA)
CREATE TABLE IF NOT EXISTS IntegrityZome (
    dna_hash        BLOB           NOT NULL,
    agent           BLOB           NOT NULL,
    zome_index      INTEGER        NOT NULL,
    zome_name       TEXT           NOT NULL,
    zome_hash       BLOB           NOT NULL,
    dependencies    JSON           NOT NULL,  -- JSON array of zome names
    PRIMARY KEY (dna_hash, agent, zome_index),
    FOREIGN KEY (dna_hash, agent) REFERENCES DnaDef(hash, agent) ON DELETE CASCADE
);

-- CoordinatorZome storage (one row per zome in a DNA)
CREATE TABLE IF NOT EXISTS CoordinatorZome (
    dna_hash        BLOB           NOT NULL,
    agent           BLOB           NOT NULL,
    zome_index      INTEGER        NOT NULL,
    zome_name       TEXT           NOT NULL,
    zome_hash       BLOB           NOT NULL,
    dependencies    JSON           NOT NULL,  -- JSON array of zome names
    PRIMARY KEY (dna_hash, agent, zome_index),
    FOREIGN KEY (dna_hash, agent) REFERENCES DnaDef(hash, agent) ON DELETE CASCADE
);

-- EntryDef storage (flattened from EntryDef struct)
-- Key is derived from EntryDefBufferKey (zome + entry_def_position)
CREATE TABLE IF NOT EXISTS EntryDef (
    key                     BLOB    PRIMARY KEY ON CONFLICT IGNORE,
    entry_def_id            TEXT    NOT NULL,  -- EntryDefId as string
    entry_def_id_type       TEXT    NOT NULL,  -- 'App', 'CapClaim', or 'CapGrant'
    visibility              TEXT    NOT NULL,  -- 'Public' or 'Private'
    required_validations    INTEGER NOT NULL
);
