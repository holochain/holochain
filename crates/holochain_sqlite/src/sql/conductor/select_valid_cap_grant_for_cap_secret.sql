SELECT
  Entry.blob
FROM
  Entry
WHERE
  Entry.cap_secret = ?1
  AND (
    SELECT
      COUNT(Action.hash)
    FROM
      Action
    WHERE
      Action.author = ?2
      AND (
        Action.original_entry_hash = Entry.hash
        OR Action.deletes_entry_hash = Entry.hash
      )
  ) = 0
