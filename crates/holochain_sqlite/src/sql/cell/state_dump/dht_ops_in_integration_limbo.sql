-- no-sql-format --
SELECT
  Action.blob as action_blob,
  Entry.blob as entry_blob,
  DhtOp.type as dht_type,
  DhtOp.hash as dht_hash,
  DhtOp.rowid as rowid
FROM
  Action
  JOIN DhtOp ON DhtOp.action_hash = Action.hash
  LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE
  when_integrated IS NULL
  AND validation_stage = 3
