-- simple select the whole table
SELECT encoded FROM (
  SELECT
    encoded AS encoded,
    -- if we dont store anything
    CASE WHEN storage_start_loc IS NULL OR storage_end_loc IS NULL
    THEN 4294967295
    -- if we have one contiguous span
    WHEN storage_start_loc <= storage_end_loc
    THEN
      -- if it is within our one span
      CASE WHEN :basis >= storage_start_loc AND :basis <= storage_end_loc
      THEN 0
      -- if it is before our one span
      WHEN :basis < storage_start_loc
      THEN min(storage_start_loc - :basis, (4294967295 - storage_end_loc) + :basis)
      -- otherwise it must be after our one span
      ELSE min(:basis - storage_end_loc, (4294967295 - :basis) + storage_start_loc)
      END
    -- if we have two logical spans (one wrapping span)
    ELSE
      -- if it is inside the covered area
      CASE WHEN :basis <= storage_end_loc OR :basis >= storage_start_loc
      THEN 0
      -- if it is in the center, uncovered area
      ELSE min(:basis - storage_end_loc, storage_start_loc - :basis)
      END
    END AS distance
    FROM p2p_agent_store
    ORDER BY distance
    LIMIT :limit
)
;
