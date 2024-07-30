ALTER TABLE
  ChainLock DROP COLUMN author;
ALTER TABLE
  ChainLock RENAME COLUMN lock TO author;
