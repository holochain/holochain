SELECT
  DhtOp.hash,
  DhtOp.type,
  Action.blob AS action_blob,
  Action.author AS author,
  Entry.blob AS entry_blob
FROM
  DhtOp
  JOIN Action ON DhtOp.action_hash = Action.hash
  LEFT JOIN Entry ON Action.entry_hash = Entry.hash
WHERE
  DhtOp.hash IN rarray(:hashes)
  AND DhtOp.when_integrated IS NOT NULL
