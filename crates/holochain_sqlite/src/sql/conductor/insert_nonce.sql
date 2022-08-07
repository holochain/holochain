-- because UPSERT isn't guaranteed to exist on our sqlite version
-- we need to fashion our own with an INSERT SELECT statement
INSERT INTO
  nonce
SELECT
  :agent AS agent,
  :nonce AS nonce
WHERE
  (
    -- count the rows that should supercede the one we're trying to insert
    SELECT
      count(rowid)
    FROM
      nonce
    WHERE
      agent = :agent
      AND nonce > :nonce
  ) = 0 -- if there are none, proceed with the insert
;
