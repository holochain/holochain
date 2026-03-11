-- check if there's a block for any reason against any of the target ids
SELECT
  COUNT(1) > 0
FROM
  BlockSpan
WHERE
  target_id IN rarray(:target_ids)
  AND start_us <= :time_us
  AND :time_us <= end_us
