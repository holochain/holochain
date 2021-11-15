SELECT
  DhtOp.hash,
  DhtOp.type,
  Header.blob AS header_blob,
  Entry.blob AS entry_blob
FROM
  DhtOp
  JOIN Header ON DhtOp.header_hash = Header.hash
  LEFT JOIN Entry ON Header.entry_hash = Entry.hash
WHERE
  DhtOp.hash = :hash
