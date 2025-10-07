SELECT
  Warrant.blob AS action_blob,
  Warrant.author AS author,
  LENGTH(Warrant.blob) AS action_size,
  0 AS entry_size,
  NULL AS entry_blob,
  DhtOp.type AS dht_type
FROM
  Warrant
  JOIN DhtOp ON DhtOp.action_hash = Warrant.hash
WHERE
  Warrant.warrantee = :author
  AND Warrant.type = :warrant_type
  AND (
    DhtOp.validation_status IS NULL
    OR DhtOp.validation_status = :status_valid
  )
