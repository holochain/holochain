SELECT
  EXISTS (
    SELECT
      1
    FROM
      DhtOp
      JOIN Action ON DhtOp.action_hash = Action.hash
    WHERE
      DhtOp.type = :op_type_register_agent_activity
      AND DhtOp.when_integrated IS NOT NULL
      AND Action.author = :author
      AND Action.seq <= :chain_filter_chain_top_action_seq
      AND DhtOp.authored_timestamp < :chain_filter_limit_conditions_until_timestamp
  ) AS has_action_below_timestamp
