-- p2p store
CREATE TABLE IF NOT EXISTS p2p_agent_store (
  -- primary key
  agent BLOB PRIMARY KEY ON CONFLICT REPLACE,
  -- encoded binary
  encoded BLOB NOT NULL,
  -- additional queryable fields extracted from encoding
  signed_at_ms INTEGER NOT NULL,
  expires_at_ms INTEGER NOT NULL,
  storage_center_loc INTEGER NOT NULL,
  -- additional queryable fields derived from encoding
  -- * for zero length arcs, these will all be null
  -- * for contiguous arcs, only start/end 1 will be set
  -- * for arcs that wrap, all four will be set
  storage_start_1 INTEGER NULL,
  storage_end_1 INTEGER NULL,
  storage_start_2 INTEGER NULL,
  storage_end_2 INTEGER NULL
);
