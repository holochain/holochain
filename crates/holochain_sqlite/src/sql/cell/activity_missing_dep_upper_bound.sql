-- Select the first header that breaks the chain
-- of headers with RegisterActivity ops
--
-- The end of the chain of activity is found by finding the lowest
-- header sequence number that is eithier missing a RegisterAgentActivity
-- op or is the last header with an op.
-- The last op will not have a RegisterAgentActivity op depending on it.
--
-- Take the minimum of the the set of headers without a RegisterAgentActivity op
-- depending on it and the set of headers with a RegisterAgentActivity op but without
-- the next RegisterAgentActivity op depending on it.
SELECT
  author,
  MIN(min_seq)
FROM
  (
    -- Find any headers that do not have a RegisterAgentActivity op
    -- that is either integrated or in integration limbo depending on them.
    SELECT
      author,
      MIN(seq) min_seq
    FROM
      Header
    WHERE
      -- Filter out any headers with a dependant op in integration limbo.
      hash NOT IN (
        SELECT
          header_hash
        FROM
          DhtOp
        WHERE
          DhtOp.type = :register_activity
          AND validation_stage = 3
          AND validation_status IS NOT NULL
      ) -- Also filter out any headers that have a dependant op
      -- that has been integrated.
      AND hash NOT IN (
        SELECT
          header_hash
        FROM
          DhtOp
        WHERE
          DhtOp.type = :register_activity
          AND when_integrated IS NOT NULL
      )
    GROUP BY
      author
    UNION
    -- Union the above set with the first header that is has a
    -- RegisterAgentActivity op but is missing the next one.
    SELECT
      author,
      IIF(MAX(seq) IS NOT NULL, MAX(seq) + 1, NULL) min_seq
    FROM
      DhtOp
      JOIN Header ON DhtOp.header_hash = Header.hash
    WHERE
      DhtOp.type = :register_activity
      AND DhtOp.validation_stage = 3
      AND DhtOp.validation_status IS NOT NULL
    GROUP BY
      author
  )
GROUP BY
  author