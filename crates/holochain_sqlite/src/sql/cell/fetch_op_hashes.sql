SELECT
    hash
FROM
    DHtOp
WHERE
    DhtOp.authored_timestamp_ms >= :from
    AND DhtOp.authored_timestamp_ms < :to
    AND (
        (
            -- non-wrapping case: everything in range
            :storage_start_loc <= :storage_end_loc
            AND (
                storage_center_loc >= :storage_start_loc
                AND storage_center_loc <= :storage_end_loc
            )
        )
        OR (
            -- wrapping case: everything not in range
            :storage_start_loc > :storage_end_loc
            AND (
                storage_center_loc < :storage_start_loc
                OR storage_center_loc > :storage_end_loc
            )
        )
    )
