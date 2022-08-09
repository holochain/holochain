DELETE FROM
  nonce
WHERE
  expires <= :now