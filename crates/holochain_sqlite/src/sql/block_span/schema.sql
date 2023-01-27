-- no-sql-format --

-- block spans
CREATE TABLE IF NOT EXISTS block_span (

  target BLOB NULL,
  reason BLOB NULL,

  start_ms INTEGER NULL,
  end_ms INTEGER NULL,

);
