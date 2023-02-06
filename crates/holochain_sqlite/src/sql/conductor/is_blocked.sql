-- count rows with a block for any reason against the target
SELECT
  COUNT(1) > 0
FROM
  BlockSpan
WHERE
  target_id = :target_id
  AND start_us <= :time_us
  AND :time_us <= end_us
