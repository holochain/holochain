-- Revert back to using dna_hash as primary key

DROP TABLE IF EXISTS CoordinatorZome;
DROP TABLE IF EXISTS IntegrityZome;
DROP TABLE IF EXISTS DnaDef;

-- Recreate original schema
CREATE TABLE IF NOT EXISTS DnaDef (
    hash            BLOB           PRIMARY KEY ON CONFLICT IGNORE,
    name            TEXT           NOT NULL,
    network_seed    TEXT           NOT NULL,
    properties      BLOB           NOT NULL,
    lineage         JSON
);

CREATE TABLE IF NOT EXISTS IntegrityZome (
    dna_hash        BLOB           NOT NULL,
    zome_index      INTEGER        NOT NULL,
    zome_name       TEXT           NOT NULL,
    wasm_hash       BLOB,
    dependencies    JSON           NOT NULL,
    PRIMARY KEY (dna_hash, zome_index),
    FOREIGN KEY (dna_hash) REFERENCES DnaDef(hash) ON DELETE CASCADE,
    FOREIGN KEY (wasm_hash) REFERENCES Wasm(hash)
);

CREATE TABLE IF NOT EXISTS CoordinatorZome (
    dna_hash        BLOB           NOT NULL,
    zome_index      INTEGER        NOT NULL,
    zome_name       TEXT           NOT NULL,
    wasm_hash       BLOB,
    dependencies    JSON           NOT NULL,
    PRIMARY KEY (dna_hash, zome_index),
    FOREIGN KEY (dna_hash) REFERENCES DnaDef(hash) ON DELETE CASCADE,
    FOREIGN KEY (wasm_hash) REFERENCES Wasm(hash)
);
