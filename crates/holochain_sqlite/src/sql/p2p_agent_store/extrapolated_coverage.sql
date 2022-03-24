SELECT
  SUM(
    -- first, sum up the 0.0 - 1.0 coverage of everyone contained in our arc
    CASE
      -- if start is before end
      WHEN (storage_start_loc <= storage_end_loc) THEN IFNULL(
        CAST(storage_end_loc AS FLOAT) - CAST(storage_start_loc AS FLOAT),
        0.0
      )
      ELSE -- else if start is after end
      IFNULL(
        4294967295.0 - CAST(storage_start_loc AS FLOAT) + CAST(storage_end_loc AS FLOAT),
        0.0
      )
    END
  ) / (
    -- then extrapolate assuming similar coverage for the rest of the arc
    CASE
      WHEN (:start_loc <= :end_loc) THEN -- if start is before end
      CAST(:end_loc AS FLOAT) - CAST(:start_loc AS FLOAT)
      ELSE -- else if start is after end
      4294967295.0 - CAST(:start_loc AS FLOAT) + CAST(:end_loc AS FLOAT)
    END
  ) AS coverage
FROM
  p2p_agent_store
WHERE
  is_active = TRUE -- only active entries
  AND expires_at_ms >= :now -- only unexpired entries
  AND (
    -- only entries in our arc type 1
    (
      :start_loc <= :end_loc
      AND storage_center_loc >= :start_loc
      AND storage_center_loc <= :end_loc
    )
    OR -- only entries in our arc type 2
    (
      :start_loc > :end_loc
      AND (
        storage_center_loc >= :start_loc
        OR storage_center_loc <= :end_loc
      )
    )
  );
