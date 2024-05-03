-- Drop index on old column
DROP INDEX DhtOp_action_hash_idx;
-- backup existing column
ALTER TABLE
  DhtOp RENAME COLUMN action_hash TO action_hash_old;
-- set null
ALTER TABLE
  DhtOp
ADD
  COLUMN action_hash BLOB NULL;
-- copy old column to new
UPDATE
  DhtOp
SET
  action_hash = action_hash_old;
-- drop old column
ALTER TABLE
  DhtOp DROP COLUMN action_hash_old;
-- Recreate index
CREATE INDEX IF NOT EXISTS DhtOp_action_hash_idx ON DhtOp ( action_hash );