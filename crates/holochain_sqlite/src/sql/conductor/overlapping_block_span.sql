-- every span that is overlapped by the given span
SELECT
    id,
    start_ms,
    end_ms
FROM
    BlockSpan
WHERE
    target_id = :target_id
    AND target_reason = :target_reason
    AND end_ms >= :start_ms
    AND start_ms <= :end_ms

    -- every span that is overlapped by the given span
INSERT INTO BlockSpan (target_id, target_reason, start_ms, end_ms)
VALUES (:target_id, :target_reason, :start_ms, :end_ms);


SELECT target_id FROM BlockSpan