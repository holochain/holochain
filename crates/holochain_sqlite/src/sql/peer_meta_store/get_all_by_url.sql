SELECT
  meta_key,
  meta_value,
  expires_at
FROM
  peer_meta
WHERE
  peer_url = :peer_url
  AND (
    expires_at IS NULL
    OR expires_at >= unixepoch() * 1000000
  );
