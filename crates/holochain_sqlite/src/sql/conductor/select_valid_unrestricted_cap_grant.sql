SELECT
  blob
FROM
  Entry
WHERE
  access_type = ?1
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
  ) = 0;
