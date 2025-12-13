-- Select Actions by the given author, where the Action has a valid RegisterAgentActivity Op
WITH ranked_actions AS (
  SELECT
    Action.seq,
    Action.hash,
    Action.blob,
    -- Partition by Action.seq so that we can exclude forked Actions with the same seq number
    ROW_NUMBER() OVER (
      PARTITION BY Action.seq
      ORDER BY
        Action.hash DESC
    ) AS row_num
  FROM
    DhtOp
    JOIN Action ON DhtOp.action_hash = Action.hash
  WHERE
    DhtOp.type = :op_type_register_agent_activity
    AND DhtOp.when_integrated IS NOT NULL
    AND Action.author = :author
    -- Action sequence numbers must be less than or equal to the sequence number of the Action with a hash matching ChainFilter `top`
    AND Action.seq <= :chain_filter_chain_top_action_seq
    -- Optionally, Action sequence numbers must be greater than or equal to the sequence number of the Action with a hash matching ChainFilter `LimitCondition::UntilHash`
    AND IIF(
      :chain_filter_limit_conditions_until_hashes_max_seq IS NOT NULL,
      Action.seq >= :chain_filter_limit_conditions_until_hashes_max_seq,
      TRUE
    )
    -- Optionally, Action timestamps must be greater than or equal to the ChainFilter LimitCondition::UntilTimestamp
    AND IIF(
      :chain_filter_limit_conditions_until_timestamp IS NOT NULL,
      DhtOp.authored_timestamp >= :chain_filter_limit_conditions_until_timestamp,
      TRUE
    )
)
-- Exclude forked actions, keeping only the first.
-- This will retain the forked Action with the maximum ActionHash.
SELECT
  *
FROM
  ranked_actions
WHERE
  row_num = 1
ORDER BY
  seq DESC
  -- Optionally, limit returned rows to the ChainFilter `take`
LIMIT
  IIF(
    :chain_filter_limit_conditions_take IS NOT NULL,
    :chain_filter_limit_conditions_take,
    -1
  )
