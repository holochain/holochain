-- no-sql-format --

-- block spans
CREATE TABLE IF NOT EXISTS BlockSpan (

  target_id BLOB NULL,
  target_reason BLOB NULL,

  start_ms INTEGER NULL,
  end_ms INTEGER NULL,

);
