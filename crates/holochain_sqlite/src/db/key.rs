use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use std::io::Error;
use std::sync::{Arc, Mutex};

const PRAGMA_LEN: usize = 220;
const PRAGMA: &[u8; PRAGMA_LEN] = br#"
PRAGMA key = "x'----------------------------------------------------------------'";
PRAGMA cipher_salt = "x'--------------------------------'";
PRAGMA cipher_compatibility = 4;
PRAGMA cipher_plaintext_header_size = 32;
"#;

pub type Result<T> = std::io::Result<T>;

/// Secure database access.
#[derive(Clone)]
pub struct DbKey {
    /// The full unlocked sqlcipher PRAGMA command set to unlock a database.
    pub unlocked: Arc<Mutex<sodoken::SizedLockedArray<PRAGMA_LEN>>>,

    /// The unlocked key.
    pub key: Arc<Mutex<sodoken::SizedLockedArray<32>>>,

    /// The salt.
    pub salt: [u8; sodoken::argon2::ARGON2_ID_SALTBYTES],

    /// The encrypted key and salt to store on disk.
    pub locked: String,
}

impl std::fmt::Debug for DbKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbKey").finish()
    }
}

impl Default for DbKey {
    fn default() -> Self {
        Self::priv_new(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABfEkbuZZnisvvyc5OIofAk1cHNw7UWbmvKbCmm3QrDjJr5Ox33KnvqRb8F7Z2fM_AAAAAAAAAAAAAAAAAAAAAA".to_string(),
            sodoken::SizedLockedArray::<32>::new().expect("Failed to allocate secure db key memory"),
            [0; 16],
        )
    }
}

impl DbKey {
    fn priv_new(
        locked: String,
        mut key: sodoken::SizedLockedArray<32>,
        salt: [u8; sodoken::argon2::ARGON2_ID_SALTBYTES],
    ) -> Self {
        let mut unlocked = sodoken::SizedLockedArray::<{ PRAGMA_LEN }>::new()
            .expect("failed to allocate secure db key memory");

        {
            let mut lock = unlocked.lock();
            lock.copy_from_slice(PRAGMA);
            for (i, b) in key.lock().iter().enumerate() {
                let c = format!("{b:02X}");
                let idx = 17 + (i * 2);
                lock[idx..idx + 2].copy_from_slice(c.as_bytes())
            }
            for (i, b) in salt.iter().enumerate() {
                let c = format!("{b:02X}");
                let idx = 109 + (i * 2);
                lock[idx..idx + 2].copy_from_slice(c.as_bytes())
            }
        }

        Self {
            unlocked: Arc::new(Mutex::new(unlocked)),
            key: Arc::new(Mutex::new(key)),
            salt,
            locked,
        }
    }

    async fn priv_gen(
        nonce: [u8; sodoken::secretbox::XSALSA_NONCEBYTES],
        mut key: sodoken::SizedLockedArray<32>,
        salt: [u8; sodoken::argon2::ARGON2_ID_SALTBYTES],
        passphrase: Arc<Mutex<sodoken::LockedArray>>,
    ) -> Result<Self> {
        let mut secret =
            tokio::task::spawn_blocking(move || -> Result<sodoken::SizedLockedArray<32>> {
                let mut secret = sodoken::SizedLockedArray::<32>::new()?;
                sodoken::argon2::blocking_argon2id(
                    &mut *secret.lock(),
                    &passphrase.lock().unwrap().lock(),
                    &salt,
                    sodoken::argon2::ARGON2_ID_OPSLIMIT_MODERATE,
                    sodoken::argon2::ARGON2_ID_MEMLIMIT_MODERATE,
                )?;

                Ok(secret)
            })
            .await??;

        let mut cipher = vec![0; key.lock().len() + sodoken::secretbox::XSALSA_MACBYTES];
        sodoken::secretbox::xsalsa_easy(&mut cipher, &nonce, &*key.lock(), &secret.lock())?;

        let mut buf = Vec::with_capacity(
            sodoken::secretbox::XSALSA_NONCEBYTES + 32 + sodoken::secretbox::XSALSA_MACBYTES + 16,
        );
        buf.extend_from_slice(&nonce);
        buf.extend_from_slice(&cipher);
        buf.extend_from_slice(&salt);

        let locked = URL_SAFE_NO_PAD.encode(&buf);

        Ok(Self::priv_new(locked, key, salt))
    }

    /// Load a database key encrypted by passphrase.
    pub async fn load(
        locked: String,
        passphrase: Arc<Mutex<sodoken::LockedArray>>,
    ) -> Result<Self> {
        let buf = URL_SAFE_NO_PAD.decode(&locked).map_err(Error::other)?;

        let mut salt = [0; sodoken::argon2::ARGON2_ID_SALTBYTES];
        salt.copy_from_slice(
            &buf[sodoken::secretbox::XSALSA_NONCEBYTES + 32 + sodoken::secretbox::XSALSA_MACBYTES
                ..sodoken::secretbox::XSALSA_NONCEBYTES
                    + 32
                    + sodoken::secretbox::XSALSA_MACBYTES
                    + 16],
        );

        let mut secret =
            tokio::task::spawn_blocking(move || -> Result<sodoken::SizedLockedArray<32>> {
                let mut secret = sodoken::SizedLockedArray::<32>::new()?;
                sodoken::argon2::blocking_argon2id(
                    &mut *secret.lock(),
                    &passphrase.lock().unwrap().lock(),
                    &salt,
                    sodoken::argon2::ARGON2_ID_OPSLIMIT_MODERATE,
                    sodoken::argon2::ARGON2_ID_MEMLIMIT_MODERATE,
                )?;

                Ok(secret)
            })
            .await??;

        let mut nonce = [0; sodoken::secretbox::XSALSA_NONCEBYTES];
        nonce.copy_from_slice(&buf[..sodoken::secretbox::XSALSA_NONCEBYTES]);

        let mut cipher = vec![0; 32 + sodoken::secretbox::XSALSA_MACBYTES];
        cipher.copy_from_slice(
            &buf[sodoken::secretbox::XSALSA_NONCEBYTES
                ..sodoken::secretbox::XSALSA_NONCEBYTES + 32 + sodoken::secretbox::XSALSA_MACBYTES],
        );

        let mut key = sodoken::SizedLockedArray::<32>::new()?;
        sodoken::secretbox::xsalsa_open_easy(&mut *key.lock(), &cipher, &nonce, &secret.lock())?;

        Ok(Self::priv_new(locked, key, salt))
    }

    /// Generate a new random database key encrypted by passphrase.
    pub async fn generate(passphrase: Arc<Mutex<sodoken::LockedArray>>) -> Result<Self> {
        let mut nonce = [0; sodoken::secretbox::XSALSA_NONCEBYTES];
        sodoken::random::randombytes_buf(&mut nonce)?;

        let mut key = sodoken::SizedLockedArray::<32>::new()?;
        sodoken::random::randombytes_buf(&mut *key.lock())?;

        let mut salt = [0; sodoken::argon2::ARGON2_ID_SALTBYTES];
        sodoken::random::randombytes_buf(&mut salt)?;

        Self::priv_gen(nonce, key, salt, passphrase).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn db_key_sanity() {
        let test1 = DbKey::default();

        let test2 = DbKey::priv_gen(
            [0; sodoken::secretbox::XSALSA_NONCEBYTES],
            sodoken::SizedLockedArray::<32>::new().unwrap(),
            [0; sodoken::argon2::ARGON2_ID_SALTBYTES],
            Arc::new(Mutex::new(sodoken::LockedArray::from(
                b"passphrase".to_vec(),
            ))),
        )
        .await
        .unwrap();

        assert_eq!(
            String::from_utf8_lossy(&*test1.unlocked.lock().unwrap().lock()),
            String::from_utf8_lossy(&*test2.unlocked.lock().unwrap().lock()),
        );

        assert_eq!(
            r#"
PRAGMA key = "x'0000000000000000000000000000000000000000000000000000000000000000'";
PRAGMA cipher_salt = "x'00000000000000000000000000000000'";
PRAGMA cipher_compatibility = 4;
PRAGMA cipher_plaintext_header_size = 32;
"#,
            &String::from_utf8_lossy(&*test2.unlocked.lock().unwrap().lock()),
        );

        let test3 = DbKey::generate(Arc::new(Mutex::new(sodoken::LockedArray::from(
            b"passphrase".to_vec(),
        ))))
        .await
        .unwrap();

        assert_ne!(
            String::from_utf8_lossy(&*test1.unlocked.lock().unwrap().lock()),
            String::from_utf8_lossy(&*test3.unlocked.lock().unwrap().lock()),
        );

        let test4 = DbKey::load(
            test3.locked,
            Arc::new(Mutex::new(sodoken::LockedArray::from(
                b"passphrase".to_vec(),
            ))),
        )
        .await
        .unwrap();

        assert_eq!(
            String::from_utf8_lossy(&*test3.unlocked.lock().unwrap().lock()),
            String::from_utf8_lossy(&*test4.unlocked.lock().unwrap().lock()),
        );
    }
}
