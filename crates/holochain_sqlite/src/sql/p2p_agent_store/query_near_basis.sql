-- simple select the whole table
SELECT encoded FROM (
  SELECT
    encoded AS encoded,
    -- if we have two spans
    CASE WHEN storage_start_1 IS NOT NULL
      AND storage_end_1 IS NOT NULL
      AND storage_start_2 IS NOT NULL
      AND storage_end_2 IS NOT NULL
    THEN
      -- if it is within the first span
      CASE WHEN :basis >= storage_start_1 AND :basis <= storage_end_1
      THEN 0
      -- if it is within the first span
      WHEN :basis >= storage_start_2 AND :basis <= storage_end_2
      THEN 0
      -- find the side it is closest to
      ELSE min(:basis - storage_end_1, storage_start_2 - :basis)
      END
    -- if we have one single span
    WHEN storage_start_1 IS NOT NULL
      AND storage_end_1 IS NOT NULL
    THEN
      -- if it is within our one span
      CASE WHEN :basis >= storage_start_1 AND :basis <= storage_end_1
      THEN 0
      -- if it is before our one span
      WHEN :basis < storage_start_1
      THEN min(storage_start_1 - :basis, (4294967295 - storage_end_1) + :basis)
      -- otherwise it must be after our one span
      ELSE min(:basis - storage_end_1, (4294967295 - :basis) + storage_start_1)
      END
    -- if we have no spans, set the distance to u32::MAX
    ELSE 4294967295
    END AS distance
    FROM p2p_agent_store
    ORDER BY distance
    LIMIT :limit
)
;
