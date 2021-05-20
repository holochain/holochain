SELECT
    hash
FROM
    DHtOp
WHERE
    DhtOp.authored_timestamp_ms >= :from
    AND DhtOp.authored_timestamp_ms < :to