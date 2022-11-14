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
