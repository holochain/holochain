use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sodoken::hash::argon2id::*;
use sodoken::secretbox::xchacha20poly1305::*;
use std::io::Error;

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
    pub unlocked: sodoken::BufRead,

    /// The unlocked key.
    pub key: sodoken::BufReadSized<32>,

    /// The salt.
    pub salt: sodoken::BufReadSized<16>,

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
            sodoken::BufReadSized::new_no_lock([0; 32]),
            sodoken::BufReadSized::new_no_lock([0; 16]),
        )
    }
}

impl DbKey {
    fn priv_new(
        locked: String,
        key: sodoken::BufReadSized<32>,
        salt: sodoken::BufReadSized<16>,
    ) -> Self {
        let unlocked = sodoken::BufWrite::new_mem_locked(PRAGMA_LEN)
            .expect("failed to allocate secure db key memory");

        {
            let mut lock = unlocked.write_lock();
            lock.copy_from_slice(PRAGMA);
            for (i, b) in key.read_lock().iter().enumerate() {
                let c = format!("{b:02X}");
                let idx = 17 + (i * 2);
                lock[idx..idx + 2].copy_from_slice(c.as_bytes())
            }
            for (i, b) in salt.read_lock().iter().enumerate() {
                let c = format!("{b:02X}");
                let idx = 109 + (i * 2);
                lock[idx..idx + 2].copy_from_slice(c.as_bytes())
            }
        }

        Self {
            unlocked: unlocked.into(),
            key,
            salt,
            locked,
        }
    }

    async fn priv_gen(
        nonce: sodoken::BufReadSized<NONCEBYTES>,
        key: sodoken::BufReadSized<32>,
        salt: sodoken::BufReadSized<16>,
        passphrase: sodoken::BufRead,
    ) -> Result<Self> {
        let secret = <sodoken::BufWriteSized<32>>::new_mem_locked()?;
        hash(
            secret.clone(),
            passphrase,
            salt.clone(),
            OPSLIMIT_MODERATE,
            MEMLIMIT_MODERATE,
        )
        .await?;

        let cipher = easy(nonce.clone(), key.clone(), secret).await?;

        let mut buf = Vec::with_capacity(NONCEBYTES + 32 + MACBYTES + 16);
        buf.extend_from_slice(&*nonce.read_lock());
        buf.extend_from_slice(&*cipher.read_lock());
        buf.extend_from_slice(&*salt.read_lock());

        let locked = URL_SAFE_NO_PAD.encode(&buf);

        Ok(Self::priv_new(locked, key, salt))
    }

    /// Load a database key encrypted by passphrase.
    pub async fn load(locked: String, passphrase: sodoken::BufRead) -> Result<Self> {
        let buf = URL_SAFE_NO_PAD.decode(&locked).map_err(Error::other)?;

        let salt = <sodoken::BufWriteSized<16>>::new_no_lock();
        salt.write_lock()
            .copy_from_slice(&buf[NONCEBYTES + 32 + MACBYTES..NONCEBYTES + 32 + MACBYTES + 16]);

        let secret = <sodoken::BufWriteSized<32>>::new_mem_locked()?;
        hash(
            secret.clone(),
            passphrase,
            salt.clone(),
            OPSLIMIT_MODERATE,
            MEMLIMIT_MODERATE,
        )
        .await?;

        let nonce = <sodoken::BufWriteSized<NONCEBYTES>>::new_no_lock();
        nonce.write_lock().copy_from_slice(&buf[..NONCEBYTES]);

        let cipher = <sodoken::BufWriteSized<{ 32 + MACBYTES }>>::new_no_lock();
        cipher
            .write_lock()
            .copy_from_slice(&buf[NONCEBYTES..NONCEBYTES + 32 + MACBYTES]);

        let key = <sodoken::BufWriteSized<32>>::new_mem_locked()?;
        open_easy(nonce, key.clone(), cipher, secret).await?;

        Ok(Self::priv_new(locked, key.into(), salt.into()))
    }

    /// Generate a new random database key encrypted by passphrase.
    pub async fn generate(passphrase: sodoken::BufRead) -> Result<Self> {
        let nonce = <sodoken::BufWriteSized<NONCEBYTES>>::new_no_lock();
        sodoken::random::bytes_buf(nonce.clone()).await?;

        let key = <sodoken::BufWriteSized<32>>::new_mem_locked()?;
        sodoken::random::bytes_buf(key.clone()).await?;

        let salt = <sodoken::BufWriteSized<16>>::new_no_lock();
        sodoken::random::bytes_buf(salt.clone()).await?;

        Self::priv_gen(nonce.into(), key.into(), salt.into(), passphrase).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn db_key_sanity() {
        let test1 = DbKey::default();

        let test2 = DbKey::priv_gen(
            sodoken::BufReadSized::new_no_lock([0; NONCEBYTES]),
            sodoken::BufReadSized::new_no_lock([0; 32]),
            sodoken::BufReadSized::new_no_lock([0; 16]),
            sodoken::BufRead::new_no_lock(b"passphrase"),
        )
        .await
        .unwrap();

        assert_eq!(
            String::from_utf8_lossy(&*test1.unlocked.read_lock()),
            String::from_utf8_lossy(&*test2.unlocked.read_lock()),
        );

        assert_eq!(
            r#"
PRAGMA key = "x'0000000000000000000000000000000000000000000000000000000000000000'";
PRAGMA cipher_salt = "x'00000000000000000000000000000000'";
PRAGMA cipher_compatibility = 4;
PRAGMA cipher_plaintext_header_size = 32;
"#,
            &String::from_utf8_lossy(&*test2.unlocked.read_lock()),
        );

        let test3 = DbKey::generate(sodoken::BufRead::new_no_lock(b"passphrase"))
            .await
            .unwrap();

        assert_ne!(
            String::from_utf8_lossy(&*test1.unlocked.read_lock()),
            String::from_utf8_lossy(&*test3.unlocked.read_lock()),
        );

        let test4 = DbKey::load(test3.locked, sodoken::BufRead::new_no_lock(b"passphrase"))
            .await
            .unwrap();

        assert_eq!(
            String::from_utf8_lossy(&*test3.unlocked.read_lock()),
            String::from_utf8_lossy(&*test4.unlocked.read_lock()),
        );
    }
}
