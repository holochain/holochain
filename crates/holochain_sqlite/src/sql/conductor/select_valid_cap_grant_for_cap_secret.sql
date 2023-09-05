SELECT
  Entry.blob
FROM
  Entry
  INNER JOIN Action ON Action.author = ?2
  AND Action.entry_hash = Entry.hash
WHERE
  Entry.cap_secret = ?1
  AND (
    -- cap grant must not have been updated or deleted
    SELECT
      COUNT(UpdateActions.hash)
    FROM
      Action AS UpdateActions
    WHERE
      UpdateActions.author = ?2
      AND (
        UpdateActions.original_entry_hash = Entry.hash
        OR UpdateActions.deletes_entry_hash = Entry.hash
      )
  ) = 0
