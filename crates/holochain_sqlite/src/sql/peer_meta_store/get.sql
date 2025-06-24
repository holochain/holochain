SELECT
  meta_value
FROM
  peer_meta
WHERE
  peer_url = :peer_url
  AND meta_key = :meta_key
  AND (
    expires_at IS NULL
    OR expires_at >= unixepoch() * 1000000
  );
