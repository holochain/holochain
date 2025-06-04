SELECT
  peer_url,
  meta_value
FROM
  peer_meta
WHERE
  meta_key = :meta_key
