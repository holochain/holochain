-- count rows with a block for any reason against the target
SELECT COUNT(1) > 0
FROM BlockSpan
WHERE
  target_id = :target_id
  AND start_ms <= :time_ms
  AND :time_ms <= end_ms
