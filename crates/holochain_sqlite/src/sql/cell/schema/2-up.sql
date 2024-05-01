ALTER TABLE DhtOp RENAME COLUMN action_hash TO action_hash_old;
ALTER TABLE DhtOp ADD COLUMN action_hash BLOB NULL;
UPDATE DhtOp SET action_hash = action_hash_old;
ALTER TABLE DhtOp REMOVE COLUMN action_hash_old;