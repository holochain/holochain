SELECT
    hash
FROM
    DHtOp
WHERE
    DhtOp.authored_timestamp_ms >= :from
    AND DhtOp.authored_timestamp_ms < :to
    AND DhtOp.when_integrated IS NOT NULL