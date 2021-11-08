-- no-sql-format --
SELECT
  Header.blob as header_blob,
  Entry.blob as entry_blob,
  DhtOp.type as dht_type,
  DhtOp.hash as dht_hash,
  DhtOp.rowid as rowid
FROM
  Header
  JOIN DhtOp ON DhtOp.header_hash = Header.hash
  LEFT JOIN Entry ON Header.entry_hash = Entry.hash
WHERE
  when_integrated IS NULL
  AND (
    (
      is_authored = 1
      AND validation_stage IS NOT NULL
      AND validation_stage < 3
    )
    OR (
      is_authored = 0
      AND (
        validation_stage IS NULL
        OR validation_stage < 3
      )
    )
  )