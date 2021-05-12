SELECT agent
FROM p2p_store
WHERE space = :space -- correct space
  AND expires_at_ms > :now -- not expired
  AND signed_at_ms >= :since_ms -- between given signed_at range
  AND signed_at_ms <= :until_ms
  AND (
    -- if the input has two ranges, check them both
    (
      :storage_start_1 IS NOT NULL
      AND :storage_end_1 IS NOT NULL
      AND :storage_start_2 IS NOT NULL
      AND :storage_end_2 IS NOT NULL
      AND (
        (
          storage_center_loc >= :storage_start_1
          AND storage_center_loc <= :storage_end_1
        )
        OR
        (
          storage_center_loc >= :storage_start_2
          AND storage_center_loc <= :storage_end_2
        )
      )
    )
    OR
    -- if the input has only one range, check it
    (
      :storage_start_1 IS NOT NULL
      AND :storage_end_1 IS NOT NULL
      AND storage_center_loc >= :storage_start_1
      AND storage_center_loc <= :storage_end_1
    )
    -- if the input has no ranges, no records will match
  )
-- filter out any entries shadowed by a newer signed_at_ms
GROUP BY space, agent HAVING signed_at_ms = max(signed_at_ms)
;
