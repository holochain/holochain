SELECT
  blob
FROM
  Action
WHERE
  author = :author
  AND type = :type
  AND entry_type = :entry_type
  AND entry_hash = :entry_hash
  AND (
    -- Agent key must not have been updated or deleted
    SELECT
      COUNT(ModifiedAction.hash)
    FROM
      Action AS ModifiedAction
    WHERE
      ModifiedAction.author = :author
      AND (
        ModifiedAction.original_entry_hash = :entry_hash
        OR ModifiedAction.deletes_entry_hash = :entry_hash
      )
  ) = 0
