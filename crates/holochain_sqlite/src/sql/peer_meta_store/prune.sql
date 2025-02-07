DELETE FROM
  peer_meta
WHERE
  expires_at < unixepoch() * 1_000_000;
