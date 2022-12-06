SELECT
  DhtOp.hash,
  DhtOp.type,
  Action.blob AS action_blob,
  Entry.blob AS entry_blob
FROM
  DhtOp
  JOIN Action ON DhtOp.action_hash = Action.hash
  LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE
  DhtOp.hash = :hash
  AND DhtOp.withhold_publish IS NULL