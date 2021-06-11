SELECT agent, storage_start_loc, storage_end_loc
FROM p2p_agent_store
WHERE signed_at_ms >= :since_ms -- between given signed_at range
  AND signed_at_ms <= :until_ms
  -- if a null range is passed in, return no result
  -- TODO: is this check actually necessary? if both are null,
  --       both cases will return false anyway.
  AND :storage_start_loc IS NOT NULL
  AND :storage_end_loc IS NOT NULL
  AND (
    (
      -- non-wrapping case: everything within the given range
      :storage_start_loc <= :storage_end_loc
      AND (
          storage_center_loc >= :storage_start_loc
          AND storage_center_loc <= :storage_end_loc
      )
    )
    OR
    (
      -- wrapping case: everything *outside* the given range
      :storage_start_loc > :storage_end_loc
      AND (
          storage_center_loc < :storage_end_loc
          OR storage_center_loc > :storage_start_loc
      )
    )
  )
;
