SELECT
  -- first, sum up the 0.0 - 1.0 coverage of everyone centered in our arc
  SUM(CASE WHEN (storage_start_loc <= storage_end_loc) THEN
    -- if start is before end
    IFNULL(
      CAST(storage_end_loc AS FLOAT) -
      CAST(storage_start_loc AS FLOAT),
      0.0
    ) / 4294967295.0
  ELSE
    -- else if start is after end
    IFNULL(
      4294967295.0 -
      CAST(storage_start_loc AS FLOAT) +
      CAST(storage_end_loc AS FLOAT),
      0.0
    ) / 4294967295.0
  END) * (
    -- then extrapolate assuming similar coverage for the rest of the arc
    4294967295.0 /
    CASE WHEN (:start_loc <= :end_loc) THEN
      -- if start is before end
      CAST(:end_loc AS FLOAT) -
      CAST(:start_loc AS FLOAT)
    ELSE
      -- else if start is after end
      4294967295.0 -
      CAST(:start_loc AS FLOAT) +
      CAST(:end_loc AS FLOAT)
    END
  ) AS coverage
FROM
  p2p_agent_store
WHERE
  -- only active entries
  is_active = TRUE
  -- only unexpired entries
  AND expires_at_ms >= :now
  AND (
    ( :start_loc <= :end_loc
      -- only entries in our arc type 1
      AND storage_center_loc >= :start_loc
      AND storage_center_loc <= :end_loc
    ) OR ( :start_loc > :end_loc
      -- only entries in our arc type 2
      AND ( storage_center_loc >= :start_loc
        OR storage_center_loc <= :end_loc
      )
    )
  );
