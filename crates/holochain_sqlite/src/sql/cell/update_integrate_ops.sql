UPDATE
  DhtOp
SET
  when_integrated = :when_integrated,
  validation_stage = NULL
WHERE
  validation_stage = 3
  AND validation_status IS NOT NULL
  AND CASE
    DhtOp."type"
    WHEN :store_entry THEN 1
    WHEN :store_element THEN 1
    WHEN :register_activity THEN (
      DhtOp.dependency IS NULL
      OR EXISTS(
        SELECT
          1
        FROM
          DhtOp AS OP_DEP
        WHERE
          OP_DEP.header_hash = DhtOp.dependency
          AND OP_DEP.when_integrated IS NOT NULL
          AND OP_DEP."type" = :register_activity
      )
    )
    WHEN :updated_content THEN (
      EXISTS(
        SELECT
          1
        FROM
          DhtOp AS OP_DEP
        WHERE
          OP_DEP.header_hash = DhtOp.dependency
          AND OP_DEP.when_integrated IS NOT NULL
          AND OP_DEP."type" = :store_entry
      )
    )
    WHEN :updated_element THEN (
      EXISTS(
        SELECT
          1
        FROM
          DhtOp AS OP_DEP
        WHERE
          OP_DEP.header_hash = DhtOp.dependency
          AND OP_DEP.when_integrated IS NOT NULL
          AND OP_DEP."type" = :store_element
      )
    )
    WHEN :deleted_by THEN (
      EXISTS(
        SELECT
          1
        FROM
          DhtOp AS OP_DEP
        WHERE
          OP_DEP.header_hash = DhtOp.dependency
          AND OP_DEP.when_integrated IS NOT NULL
          AND OP_DEP."type" = :store_element
      )
    )
    WHEN :deleted_entry_header THEN (
      EXISTS(
        SELECT
          1
        FROM
          DhtOp AS OP_DEP
        WHERE
          OP_DEP.header_hash = DhtOp.dependency
          AND OP_DEP.when_integrated IS NOT NULL
          AND OP_DEP."type" = :store_entry
      )
    )
    WHEN :create_link THEN (
      EXISTS(
        SELECT
          1
        FROM
          DhtOp AS OP_DEP
        WHERE
          OP_DEP.basis_hash = DhtOp.dependency
          AND OP_DEP.when_integrated IS NOT NULL
          AND OP_DEP."type" = :store_entry
      )
    )
    WHEN :delete_link THEN (
      EXISTS(
        SELECT
          1
        FROM
          DhtOp AS OP_DEP
        WHERE
          OP_DEP.header_hash = DhtOp.dependency
          AND OP_DEP.when_integrated IS NOT NULL
          AND OP_DEP."type" = :create_link
      )
    )
  END
