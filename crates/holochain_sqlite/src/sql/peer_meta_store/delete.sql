DELETE FROM
  peer_meta
WHERE
  peer_url = :peer_url
  AND meta_key = :meta_key;
