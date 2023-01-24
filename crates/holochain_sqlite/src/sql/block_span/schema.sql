-- no-sql-format --

-- block spans
CREATE TABLE IF NOT EXISTS block_span (

  ipv4 BLOB NULL,
  node BLOB NULL,
  dna BLOB NULL,
  agent BLOB NULL,

  start_ms INTEGER NULL,
  end_ms INTEGER NULL,

  mut BOOLEAN NOT NULL,

);
