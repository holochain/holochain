use holochain_secure_primitive::secure_primitive;
use holochain_timestamp::Timestamp;
use std::{error::Error, time::Duration};

/// 256 Bit generic nonce.
#[derive(Clone, Copy)]
pub struct Nonce256Bits([u8; 32]);
secure_primitive!(Nonce256Bits, 32);

impl Nonce256Bits {
    pub fn into_inner(self) -> [u8; 32] {
        self.0
    }
}

/// Rather arbitrary but we expire nonces after 5 mins.
pub const FRESH_NONCE_EXPIRES_AFTER: Duration = Duration::from_secs(60 * 5);

/// Generate a fresh nonce.
///
/// The nonce will be valid from the given `now` timestamp until `now` + [FRESH_NONCE_EXPIRES_AFTER].
/// A new nonce and the expiry are returned as a tuple.
///
/// Note: the expiry isn't managed by this function. It's up to the caller to enforce the expiry
/// time of the nonce.
pub fn fresh_nonce(
    now: Timestamp,
) -> Result<(Nonce256Bits, Timestamp), Box<dyn Error + std::marker::Send + Sync>> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes)?;
    let nonce = Nonce256Bits::from(bytes);
    let expires: Timestamp = (now + FRESH_NONCE_EXPIRES_AFTER)?;
    Ok((nonce, expires))
}

#[cfg(test)]
pub mod test {
    use holochain_timestamp::Timestamp;

    use crate::{fresh_nonce, FRESH_NONCE_EXPIRES_AFTER};

    #[test]
    fn test_fresh_nonce() {
        let now = Timestamp::now();
        let (nonce, expires) = fresh_nonce(now).unwrap();
        let (nonce_2, expires_2) = fresh_nonce(now).unwrap();
        assert!(nonce != nonce_2);
        assert_eq!(expires, expires_2);
        assert_eq!(expires, (now + FRESH_NONCE_EXPIRES_AFTER).unwrap());
    }
}
