SELECT
  meta_value
FROM
  peer_meta
WHERE
  peer_url = :peer_url
  AND meta_key = :meta_key
  AND (
    expires_at IS NULL
    or expires_at >= unixepoch() * 1_000_000
  );
