DELETE FROM
  peer_meta
WHERE
  peer_url IN rarray(?1)
