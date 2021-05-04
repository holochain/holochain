CREATE TABLE IF NOT EXISTS AgentInfo (
    key             BLOB           PRIMARY KEY ON CONFLICT REPLACE,
    blob            BLOB           NOT NULL,
);