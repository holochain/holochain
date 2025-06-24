-- Add new column for the serialized size of the DhtOp
ALTER TABLE
  DhtOp
ADD
  COLUMN serialized_size INTEGER NOT NULL DEFAULT 0;

-- Populate the serialized_size column with approximate values
UPDATE
  DhtOp
SET
  serialized_size = (
    SELECT
      LENGTH(blob)
    FROM
      Action
    WHERE
      Action.hash = DhtOp.action_hash
  );

CREATE TABLE SliceHash (
  arc_start INTEGER NOT NULL,
  arc_end INTEGER NOT NULL,
  slice_index INTEGER NOT NULL,
  hash BLOB NOT NULL,
  PRIMARY KEY (arc_start, arc_end, slice_index) ON CONFLICT REPLACE
);
