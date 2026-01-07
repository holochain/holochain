-- Update DnaDef table to use cell_id as primary key instead of hash
-- This allows multiple cells with the same DNA hash but different coordinator zomes

-- First, drop the existing foreign key constraints that reference DnaDef.hash
DROP TABLE IF EXISTS CoordinatorZome;
DROP TABLE IF EXISTS IntegrityZome;
DROP TABLE IF EXISTS DnaDef;

-- Recreate DnaDef with cell_id as primary key
CREATE TABLE IF NOT EXISTS DnaDef (
    cell_id         BLOB           PRIMARY KEY ON CONFLICT IGNORE,  -- CellId (dna_hash + agent_key)
    dna_hash        BLOB           NOT NULL,                       -- DnaHash for reference
    name            TEXT           NOT NULL,
    network_seed    TEXT           NOT NULL,
    properties      BLOB           NOT NULL,  -- SerializedBytes
    lineage         JSON                      -- JSON HashSet<DnaHash>
);

-- Create an index on dna_hash for efficient lookups
CREATE INDEX idx_dna_def_hash ON DnaDef(dna_hash);

-- Recreate IntegrityZome with reference to cell_id
CREATE TABLE IF NOT EXISTS IntegrityZome (
    cell_id         BLOB           NOT NULL,
    zome_index      INTEGER        NOT NULL,
    zome_name       TEXT           NOT NULL,
    wasm_hash       BLOB,                     -- NULL for inline zomes
    dependencies    JSON           NOT NULL,  -- JSON array of zome names
    PRIMARY KEY (cell_id, zome_index),
    FOREIGN KEY (cell_id) REFERENCES DnaDef(cell_id) ON DELETE CASCADE,
    FOREIGN KEY (wasm_hash) REFERENCES Wasm(hash)
);

-- Recreate CoordinatorZome with reference to cell_id
CREATE TABLE IF NOT EXISTS CoordinatorZome (
    cell_id         BLOB           NOT NULL,
    zome_index      INTEGER        NOT NULL,
    zome_name       TEXT           NOT NULL,
    wasm_hash       BLOB,                     -- NULL for inline zomes
    dependencies    JSON           NOT NULL,  -- JSON array of zome names
    PRIMARY KEY (cell_id, zome_index),
    FOREIGN KEY (cell_id) REFERENCES DnaDef(cell_id) ON DELETE CASCADE,
    FOREIGN KEY (wasm_hash) REFERENCES Wasm(hash)
);
