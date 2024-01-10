-- See how many times ops have been retried in app validation
-- select
--     DhtOp.num_validation_attempts,
--     count(*)
-- FROM
--     DhtOp
-- where
--     DhtOp.when_integrated IS NULL
--     AND DhtOp.validation_status IS NULL
--     AND (
--         DhtOp.validation_stage = 1
--         OR DhtOp.validation_stage = 2
--     )
-- group by
--     DhtOp.num_validation_attempts;

-- Find an author who has produced a lot of ops
SELECT
    hex(Action.author),
    count(*)
FROM
    DhtOp
LEFT JOIN Action ON DhtOp.action_hash = Action.hash
WHERE
    DhtOp.when_integrated IS NULL
    AND DhtOp.validation_status IS NULL
    AND (
        DhtOp.validation_stage = 1
        OR DhtOp.validation_stage = 2
    )
GROUP BY
    Action.author;
