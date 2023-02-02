-- no-sql-format --

CREATE TABLE IF NOT EXISTS ConductorState (
    id              INTEGER        PRIMARY KEY ON CONFLICT REPLACE,
    blob            BLOB           NOT NULL
);

CREATE TABLE IF NOT EXISTS Nonce (
    -- Primary key
    agent BLOB PRIMARY KEY ON CONFLICT REPLACE,
    nonce BLOB NOT NULL,
    expires INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS BlockSpan (
    id INTEGER PRIMARY KEY,

    target_id BLOB NOT NULL,
    target_reason BLOB NOT NULL,

    start_ms INTEGER NOT NULL,
    end_ms INTEGER NOT NULL
);