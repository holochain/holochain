-- no-sql-format --

CREATE TABLE IF NOT EXISTS Nonce (
    -- Primary key
    agent            BLOB      PRIMARY KEY ON CONFLICT REPLACE,
    nonce            INTEGER           NOT NULL
);
