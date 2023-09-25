SELECT
  Entry.blob
FROM
  Entry
  INNER JOIN Action ON Action.author = ?2
  AND Action.entry_hash = Entry.hash
WHERE
  access_type = ?1
  AND (
    -- cap grant must not have been updated or deleted
    SELECT
      COUNT(UpdateActions.hash)
    FROM
      Action as UpdateActions
    WHERE
      UpdateActions.author = ?2
      AND (
        UpdateActions.original_entry_hash = Entry.hash
        OR UpdateActions.deletes_entry_hash = Entry.hash
      )
  ) = 0;
