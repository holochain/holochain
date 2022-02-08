WITH ready_to_integrate AS (
  SELECT
    header_hash
  FROM
    DhtOp
  WHERE
    DhtOp.type = :register_activity
    AND DhtOp.validation_stage = 3
    AND DhtOp.validation_status IS NOT NULL
),
already_integrated AS (
  SELECT
    header_hash
  FROM
    DhtOp
  WHERE
    DhtOp.type = :register_activity
    AND DhtOp.validation_stage IS NULL
    AND DhtOp.when_integrated IS NOT NULL
)
SELECT
  Header.author,
  MIN(Header.seq) min_seq
FROM
  Header
  LEFT JOIN Header NextHeader ON NextHeader.prev_hash = Header.hash
WHERE
  Header.hash IN (
    SELECT
      *
    FROM
      ready_to_integrate
  )
  AND (
    NextHeader.hash IS NULL
    OR NextHeader.hash NOT IN (
      SELECT
        *
      FROM
        ready_to_integrate
    )
  )
  AND (
    Header.prev_hash IS NULL
    OR Header.prev_hash IN (
      SELECT
        *
      FROM
        ready_to_integrate
    )
    OR Header.prev_hash IN (
      SELECT
        *
      FROM
        already_integrated
    )
  )
GROUP BY
  Header.author