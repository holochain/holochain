FROM
  BlockSpan
WHERE
  target_id = :target_id
  AND target_reason = :target_reason
  AND (
    start_ms <= :end_ms
    AND end_ms >= :start_ms
  )
