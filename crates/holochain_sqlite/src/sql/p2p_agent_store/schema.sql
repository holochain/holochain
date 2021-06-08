-- p2p store
CREATE TABLE IF NOT EXISTS p2p_agent_store (
  -- Primary key
  agent                   BLOB      PRIMARY KEY ON CONFLICT REPLACE,

  -- Encoded binary
  encoded                 BLOB      NOT NULL,

  -- Additional queryable fields extracted from encoding
  signed_at_ms            INTEGER   NOT NULL,
  expires_at_ms           INTEGER   NOT NULL,
  storage_center_loc      INTEGER   NOT NULL,

  -- Additional queryable fields derived from encoding:
  -- For zero length arcs, these will both be NULL.
  -- Otherwise, both will be set, i.e. XOR of these two fields is always false.
  -- If the start loc is greater than the end loc, then this represents a
  -- "wrapping" range
  storage_start_loc         INTEGER   NULL,
  storage_end_loc           INTEGER   NULL,
);
