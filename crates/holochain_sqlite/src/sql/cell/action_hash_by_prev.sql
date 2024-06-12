SELECT
  hash,
  blob
FROM
  Action
WHERE
  prev_hash = :prev_hash
  AND CASE
    WHEN -- When this is an Update action for an Agent key...
    entry_type = :entry_type_agent THEN -- ...the prev action's author will be different from the following action's author,
    -- so we need to check the entry_hash, which determines the new agent key going forward.
    -- Create actions will have the same author and entry_hash, so it doesn't hurt to
    -- include them in this special case.
    entry_hash = :author_entry_hash
    ELSE -- Otherwise, we can just check the author.
    author = :author
  END
LIMIT
  1
