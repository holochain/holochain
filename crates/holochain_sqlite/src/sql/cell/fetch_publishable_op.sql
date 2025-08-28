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
  DhtOp.hash = :hash
  AND DhtOp.withhold_publish IS NULL
UNION
ALL
SELECT
  DhtOp.hash,
  DhtOp.type,
  Warrant.blob AS action_blob,
  Warrant.author AS author,
  NULL AS entry_blob
FROM
  DhtOp
  JOIN Warrant ON DhtOp.action_hash = Warrant.hash
WHERE
  DhtOp.hash = :hash
  AND DhtOp.withhold_publish IS NULL
