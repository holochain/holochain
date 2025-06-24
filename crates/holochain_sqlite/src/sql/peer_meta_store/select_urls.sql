SELECT
  peer_url,
  meta_value
FROM
  peer_meta
WHERE
  meta_key = :meta_key
  AND (
    expires_at IS NULL
    OR expires_at >= unixepoch() * 1000000
  );
