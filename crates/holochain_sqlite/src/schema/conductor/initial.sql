CREATE TABLE IF NOT EXISTS ConductorState (
    id              INTEGER        PRIMARY KEY ON CONFLICT REPLACE,
    blob            BLOB           NOT NULL,
);