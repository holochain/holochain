-- Add new column for the serialized size of the DhtOp
ALTER TABLE
  DhtOp
ADD
  COLUMN serialized_size INTEGER NOT NULL DEFAULT 0;

-- Populate the serialized_size column with approximate values
UPDATE
  DhtOp
set
  serialized_size = (
    SELECT
      LENGTH(blob)
    from
      Action
    where
      Action.hash = DhtOp.action_hash
  );
