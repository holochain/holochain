-- the overall bounds across every span that is overlapped by the given span
DELETE FROM
  BlockSpan
WHERE
  target_id = :target_id
  AND target_reason = :target_reason
  AND (
    end_ms >= :start_ms
    OR start_ms <= :end_ms
  )
