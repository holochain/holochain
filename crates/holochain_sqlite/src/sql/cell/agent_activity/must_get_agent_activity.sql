-- Select Actions by the given author, where the Action has a valid RegisterAgentActivity Op
SELECT
  Action.seq,
  Action.hash,
  Action.blob,
  -- Ensures that the GROUP BY Action.seq will retain the action with the maximum ActionHash, excluding the rest
  -- This ensures that de-duplication of forked actions is determanisitic within a single query.
  MAX(Action.hash) AS max_action_hash
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
    1 = 1
  )
  -- Optionally, Action timestamps must be greater than or equal to the ChainFilter LimitCondition::UntilTimestamp
  AND IIF(
    :chain_filter_limit_conditions_until_timestamp IS NOT NULL,
    DhtOp.authored_timestamp >= :chain_filter_limit_conditions_until_timestamp,
    1 = 1
  )
  -- Exclude forked actions, keeping only the first.
  -- Because we are selecting the aggregate function MAX(Action.hash), this will retain the Action with the maximum ActionHash.
  -- This ensures that de-duplication of forked actions is determanisitic within a single query.
GROUP BY
  Action.seq
  -- Order by seq number, then hash, descending
ORDER BY
  Action.seq DESC
  -- Optionally, limit returned rows to the ChainFilter `take`
LIMIT
  IIF(
    :chain_filter_limit_conditions_take IS NOT NULL,
    :chain_filter_limit_conditions_take,
    -1
  )
