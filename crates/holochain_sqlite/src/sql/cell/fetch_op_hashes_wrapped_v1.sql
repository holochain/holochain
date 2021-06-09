-- Use this query only in the wrapping arc case,
-- i.e. when :storage_start_loc > :storage_end_loc
--
-- This is one version of this query. There is another version which may be faster.

SELECT
    hash
FROM
    DHtOp
WHERE
    DhtOp.authored_timestamp_ms >= :from
    AND DhtOp.authored_timestamp_ms < :to
    AND (
        storage_center_loc < :storage_start_loc
        OR storage_center_loc > :storage_end_loc
    )
