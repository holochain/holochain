--- Select Actions by the given author, where the Action has a valid RegisterAgentActivity Op --
SELECT
    Action.hash,
    Action.blob
FROM
    DhtOp
    JOIN Action ON DhtOp.action_hash = Action.hash
WHERE
    DhtOp.type = :op_type_register_agent_activity

    --- RegisterAgentActivity Op has been validated as valid ---
    AND DhtOp.when_integrated IS NOT NULL
    AND Action.author = :author

    --- Action sequence numbers must be less than or equal to the sequence number of the Action with a hash matching ChainFilter `top` ---
    AND Action.seq <=
    (
        SELECT
            seq
        FROM
            Action
        JOIN DhtOp ON DhtOp.action_hash = Action.hash
        WHERE
            Action.hash = :chain_filter_chain_top
            AND DhtOp.type = :op_type_register_agent_activity
            AND DhtOp.when_integrated IS NOT NULL
            AND Action.author = :author
    )

--- Optionally, Action sequence numbers must be greater than or equal to the sequence number of the Action with a hash matching ChainFilter `LimitCondition::UntilHash` ---
AND 
    IFF(
        :chain_filter_limit_conditions_until_hashes IS NOT NULL,
        Action.seq >= :chain_filter_limit_conditions_until_hashes_max_seq,
        1=1
    ) 

--- Optionally, Action timestamps must be greater than or equal to the ChainFilter LimitCondition::UntilTimestamp ---
AND 
    IFF(
        :chain_filter_limit_conditions_until_timestamp IS NOT NULL, 
        Action.timestamp >= :chain_filter_limit_conditions_until_timestamp,
        1=1
    )

--- Order by seq number, then hash, descending --
ORDER BY
    Action.seq DESC,

    --- Ordering by hash ensures that forking Actions are still ordered consistently ---
    Action.hash DESC


--- Optionally, limit returned rows to the ChainFilter `take` ---
IFF(
    :chain_filter_limit_conditions_take IS NOT NULL,
    LIMIT :chain_filter_limit_conditions_take
)
