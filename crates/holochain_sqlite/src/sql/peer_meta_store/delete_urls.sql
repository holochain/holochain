DELETE FROM
  peer_meta
WHERE
  peer_url IN rarray(:urls)
  AND meta_key = :meta_key
