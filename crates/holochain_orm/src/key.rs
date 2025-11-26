use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use std::io::Error;
use std::sync::{Arc, Mutex};

pub type Result<T> = std::io::Result<T>;

/// Secure database access.
#[derive(Clone)]
pub struct DbKey {
    /// The unlocked key.
    pub key: Arc<Mutex<sodoken::SizedLockedArray<32>>>,

    /// The salt.
    pub salt: Arc<Mutex<sodoken::SizedLockedArray<16>>>,

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
            Arc::new(Mutex::new(
                sodoken::SizedLockedArray::<16>::new().expect("Failed to allocate secure salt memory")
            )),
        )
    }
}

impl DbKey {
    fn priv_new(
        locked: String,
        key: sodoken::SizedLockedArray<32>,
        salt: Arc<Mutex<sodoken::SizedLockedArray<16>>>,
    ) -> Self {
        Self {
            key: Arc::new(Mutex::new(key)),
            salt,
            locked,
        }
    }

    async fn priv_gen(
        nonce: [u8; sodoken::secretbox::XSALSA_NONCEBYTES],
        mut key: sodoken::SizedLockedArray<32>,
        salt: Arc<Mutex<sodoken::SizedLockedArray<16>>>,
        passphrase: Arc<Mutex<sodoken::LockedArray>>,
    ) -> Result<Self> {
        let salt_clone = Arc::clone(&salt);
        let mut secret =
            tokio::task::spawn_blocking(move || -> Result<sodoken::SizedLockedArray<32>> {
                let mut secret = sodoken::SizedLockedArray::<32>::new()?;
                sodoken::argon2::blocking_argon2id(
                    &mut *secret.lock(),
                    &passphrase.lock().unwrap().lock(),
                    &*salt_clone.lock().unwrap().lock(),
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
        buf.extend_from_slice(&*salt.lock().unwrap().lock());

        let locked = URL_SAFE_NO_PAD.encode(&buf);

        Ok(Self {
            key: Arc::new(Mutex::new(key)),
            salt,
            locked,
        })
    }

    /// Load a database key encrypted by passphrase.
    pub async fn load(
        locked: String,
        passphrase: Arc<Mutex<sodoken::LockedArray>>,
    ) -> Result<Self> {
        let buf = URL_SAFE_NO_PAD.decode(&locked).map_err(Error::other)?;

        let mut salt = sodoken::SizedLockedArray::<16>::new()?;
        salt.lock().copy_from_slice(
            &buf[sodoken::secretbox::XSALSA_NONCEBYTES + 32 + sodoken::secretbox::XSALSA_MACBYTES
                ..sodoken::secretbox::XSALSA_NONCEBYTES
                    + 32
                    + sodoken::secretbox::XSALSA_MACBYTES
                    + 16],
        );

        let salt = Arc::new(Mutex::new(salt));
        let salt_clone = Arc::clone(&salt);
        let mut secret =
            tokio::task::spawn_blocking(move || -> Result<sodoken::SizedLockedArray<32>> {
                let mut secret = sodoken::SizedLockedArray::<32>::new()?;
                sodoken::argon2::blocking_argon2id(
                    &mut *secret.lock(),
                    &passphrase.lock().unwrap().lock(),
                    &*salt_clone.lock().unwrap().lock(),
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

        Ok(Self {
            key: Arc::new(Mutex::new(key)),
            salt,
            locked,
        })
    }

    /// Generate a new random database key encrypted by passphrase.
    pub async fn generate(passphrase: Arc<Mutex<sodoken::LockedArray>>) -> Result<Self> {
        let mut nonce = [0; sodoken::secretbox::XSALSA_NONCEBYTES];
        sodoken::random::randombytes_buf(&mut nonce)?;

        let mut key = sodoken::SizedLockedArray::<32>::new()?;
        sodoken::random::randombytes_buf(&mut *key.lock())?;

        let mut salt = sodoken::SizedLockedArray::<16>::new()?;
        sodoken::random::randombytes_buf(&mut *salt.lock())?;
        let salt = Arc::new(Mutex::new(salt));

        Self::priv_gen(nonce, key, salt, passphrase).await
    }

    /// Format the key as a hex string for use in PRAGMA statements.
    pub(crate) fn key_hex(&self) -> String {
        self.key
            .lock()
            .unwrap()
            .lock()
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<String>()
    }

    /// Format the salt as a hex string for use in PRAGMA statements.
    pub(crate) fn salt_hex(&self) -> String {
        self.salt
            .lock()
            .unwrap()
            .lock()
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<String>()
    }

    /// Apply encryption pragmas to SQLite connection options.
    pub(crate) fn apply_pragmas(
        &self,
        opts: sqlx::sqlite::SqliteConnectOptions,
    ) -> sqlx::sqlite::SqliteConnectOptions {
        opts.pragma("key", format!("\"x'{}'\"", self.key_hex()))
            .pragma("cipher_salt", format!("\"x'{}'\"", self.salt_hex()))
            .pragma("cipher_compatibility", "4")
            .pragma("cipher_plaintext_header_size", "32")
    }
}
