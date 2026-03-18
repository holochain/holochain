SELECT
  Action.hash,
  Action.seq,
  Action.blob
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
  AND (
    :chain_filter_limit_conditions_until_hashes_max_seq IS NULL
    OR Action.seq >= :chain_filter_limit_conditions_until_hashes_max_seq
  )
  -- Optionally, Action timestamps must be greater than or equal to the ChainFilter LimitCondition::UntilTimestamp
  AND (
    :chain_filter_limit_conditions_until_timestamp IS NULL
    OR DhtOp.authored_timestamp >= :chain_filter_limit_conditions_until_timestamp
  )
ORDER BY
  Action.seq DESC,
  Action.hash DESC
