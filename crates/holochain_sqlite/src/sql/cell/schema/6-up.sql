CREATE TABLE Warrant (
  hash BLOB PRIMARY KEY ON CONFLICT IGNORE,
  author BLOB NOT NULL,
  timestamp INTEGER NOT NULL,
  warrantee BLOB NOT NULL,
  type TEXT NOT NULL,
  blob BLOB NOT NULL
);

-- Drop foreign key constraint on action_hash in DhtOp.
-- To achieve that, create a new table without it and drop the old table.
CREATE TABLE IF NOT EXISTS DhtOpNew (
  hash BLOB PRIMARY KEY ON CONFLICT IGNORE,
  type TEXT NOT NULL,
  basis_hash BLOB NOT NULL,
  require_receipt INTEGER NOT NULL,  -- BOOLEAN
  -- This is not strictly an action hash, but a reference to a row in the Action or Warrant table. Note this is not a foreign key, because a foreign key must reference a single table, and this may reference either table, depending on if the Op is a Warrant or other Op.
  action_hash BLOB NOT NULL,
  storage_center_loc INTEGER NOT NULL,
  -- The timestamp on the DhtOp itself. NOT the timestamp of the row being created.
  authored_timestamp INTEGER NOT NULL,  -- This is the order that process ops should result
  -- in dependencies before dependants.
  -- See OpOrder.
  op_order TEXT NOT NULL,
  -- If this is null then validation is still in progress.
  validation_status INTEGER NULL,
  when_stored INTEGER NULL,  -- DATETIME. Really should be NOT NULL but no default is sensible given the need to migrate data.
  when_sys_validated INTEGER NULL,  -- DATETIME
  when_app_validated INTEGER NULL,  -- DATETIME
  when_integrated INTEGER NULL,  -- DATETIME
  -- Used to withhold ops from publishing for things
  -- like countersigning.
  withhold_publish INTEGER NULL,  -- BOOLEAN
  -- The op has received enough validation receipts.
  -- This is required as a field because different ops have different EntryTypes,
  -- which have different numbers of required validation receipts.
  receipts_complete INTEGER NULL,  -- BOOLEAN
  last_publish_time INTEGER NULL,  -- UNIX TIMESTAMP SECONDS
  -- 0: Awaiting System Validation Dependencies.
  -- 1: Successfully System Validated (And ready for app validation).
  -- 2: Awaiting App Validation Dependencies.
  -- 3: Awaiting integration.
  -- Don't need the other stages (pending, awaiting integration) because:
  -- - pending = validation_stage null && validation_status null.
  -- We could make this an enum and use a Blob so we can capture which
  -- deps are being awaited for debugging.
  validation_stage INTEGER NULL,
  num_validation_attempts INTEGER NULL,
  last_validation_attempt INTEGER NULL,
  -- The sys validation dependency if there is one.
  dependency BLOB NULL,
  -- To be deleted right after migration.
  dependency2 BLOB NULL,
  serialized_size INTEGER NOT NULL DEFAULT 0,
  transfer_source BLOB NULL,
  transfer_method INTEGER NULL,
  transfer_time INTEGER NULL
);

CREATE INDEX IF NOT EXISTS DhtOp_type_dep_idx ON DhtOpNew (type, dependency);

CREATE INDEX IF NOT EXISTS DhtOp_type_when_int_idx ON DhtOpNew (type, when_integrated);

CREATE INDEX IF NOT EXISTS DhtOp_validation_stage_idx ON DhtOpNew (validation_stage, type, dependency);

CREATE INDEX IF NOT EXISTS DhtOp_stage_type_status_idx ON DhtOpNew (validation_stage, type, validation_status);

CREATE INDEX IF NOT EXISTS DhtOp_validation_status_idx ON DhtOpNew (validation_status);

CREATE INDEX IF NOT EXISTS DhtOp_authored_timestamp_idx ON DhtOpNew (authored_timestamp);

CREATE INDEX IF NOT EXISTS DhtOp_storage_center_loc_idx ON DhtOpNew (storage_center_loc);

CREATE INDEX IF NOT EXISTS DhtOp_action_hash_idx ON DhtOpNew (action_hash);

CREATE INDEX IF NOT EXISTS DhtOp_basis_hash_idx ON DhtOpNew (basis_hash);

-- Copy data from old to new DhtOp table.
INSERT INTO
  DhtOpNew
SELECT
  *
FROM
  DhtOp;

-- Drop old table.
DROP TABLE DhtOp;

-- Rename new table.
ALTER TABLE
  DhtOpNew RENAME TO DhtOp;

-- Drop deprecated columns.
ALTER TABLE
  DhtOp DROP COLUMN dependency2;
