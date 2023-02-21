ALTER TABLE
  ValidationReceipt RENAME TO ValidationReceipt_1Up;
CREATE TABLE ValidationReceipt (
  hash BLOB PRIMARY KEY ON CONFLICT IGNORE,
  op_hash BLOB NOT NULL,
  blob BLOB NOT NULL,
  FOREIGN KEY(op_hash) REFERENCES DhtOp(hash) ON DELETE CASCADE
);
INSERT INTO
  ValidationReceipt (hash, op_hash, blob)
SELECT
  hash,
  op_hash,
  blob
FROM
  ValidationReceipt_1Up;
DROP TABLE ValidationReceipt_1Up;
