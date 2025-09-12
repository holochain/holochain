-- check if there's a block for any reason against all target ids
SELECT
  CASE
    WHEN COUNT(DISTINCT target_id) = :ids_len THEN 1
    ELSE 0
  END
FROM
  BlockSpan
WHERE
  target_id IN rarray(:target_ids)
  AND start_us <= :time_us
  AND :time_us <= end_us
