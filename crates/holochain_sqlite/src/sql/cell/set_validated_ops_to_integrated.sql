UPDATE
  DhtOp
SET
  when_integrated = :when_integrated,
  validation_stage = NULL
WHERE
  validation_stage = 3
  AND validation_status IS NOT NULL
RETURNING
  hash,
  basis_hash,
  authored_timestamp,
  validation_status,
  -- Return the action author when the op is an action.
  (
    SELECT
      author
    FROM
      Action
    WHERE
      Action.hash = DhtOp.action_hash
    LIMIT
      1
  ),
  -- Return author from the Warrant table for warrant ops.
  (
    SELECT
      author
    FROM
      Warrant
    WHERE
      Warrant.hash = DhtOp.action_hash
    LIMIT
      1
  ),
  -- Return warrantee from the Warrant table for warrant ops.
  (
    SELECT
      warrantee
    FROM
      Warrant
    WHERE
      Warrant.hash = DhtOp.action_hash
    LIMIT
      1
  )
