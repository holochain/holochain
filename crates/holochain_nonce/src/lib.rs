use holochain_secure_primitive::secure_primitive;
use kitsune_p2p_timestamp::Timestamp;
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

pub fn fresh_nonce(
    now: Timestamp,
) -> Result<(Nonce256Bits, Timestamp), Box<dyn Error + std::marker::Send + Sync>> {
    let mut bytes = [0; 32];
    getrandom::getrandom(&mut bytes)?;
    let nonce = Nonce256Bits::from(bytes);
    let expires: Timestamp = (now + FRESH_NONCE_EXPIRES_AFTER)?;
    Ok((nonce, expires))
}

#[cfg(test)]
pub mod test {
    use kitsune_p2p_timestamp::Timestamp;

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
