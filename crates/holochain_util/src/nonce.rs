use getrandom;

pub fn fresh_nonce(now: Timestamp) -> Result<(Nonce256Bits, Timestamp)> {
  let mut bytes = [0; 32];
  getrandom::getrandom(&mut bytes)?;
  let nonce = Nonce256Bits::from(bytes);
  let expires: Timestamp = (now + FRESH_NONCE_EXPIRES_AFTER)?;
  Ok((nonce, expires))
}
