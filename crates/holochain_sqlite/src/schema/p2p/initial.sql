-- p2p store
-- the primary key is (space, agent, signed_at_ms)
-- since our version of sqlite doesn't have upsert,
-- we're just inserting always, and filtering out old signed_at_ms on selects
-- the prune will prune all expired things and shadowed things
CREATE TABLE IF NOT EXISTS p2p_store (
  -- primary key items
  space                   BLOB      NOT NULL,
  agent                   BLOB      NOT NULL,
  signed_at_ms            INTEGER   NOT NULL,

  -- encoded binary
  encoded                 BLOB      NOT NULL,

  -- additional queryable fields
  expires_at_ms           INTEGER   NOT NULL,
  storage_center_loc      INTEGER   NOT NULL,
  storage_half_length     INTEGER   NOT NULL,
  storage_start_1         INTEGER   NULL,
  storage_end_1           INTEGER   NULL,
  storage_start_2         INTEGER   NULL,
  storage_end_2           INTEGER   NULL,

  -- pk constraint
  CONSTRAINT p2p_store_pk PRIMARY KEY (
    space, agent, signed_at_ms
  ) ON CONFLICT REPLACE
);

-- this index is used for queries / selects by dht storage arc
CREATE INDEX IF NOT EXISTS p2p_store_arc_search_idx ON p2p_store (
  space, agent, signed_at_ms,
  storage_start_1, storage_end_1,
  storage_start_2, storage_end_2
);

-- this index is used by prune
CREATE INDEX IF NOT EXISTS p2p_store_expires_at_idx ON p2p_store (
  expires_at_ms
);
