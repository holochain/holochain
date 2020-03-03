use sx_dpki::{
    key_blob::{BlobType, Blobbable, KeyBlob},
    key_bundle::KeyBundle,
    keypair::{EncryptingKeyPair, KeyPair, SigningKeyPair},
    seed::Seed,
    utils::{
        decrypt_with_passphrase_buf, encrypt_with_passphrase_buf, generate_derived_seed_buf,
        generate_random_buf, SeedContext,
    },
    SEED_SIZE,
};
use holochain_locksmith::Mutex;
use sx_types::{
    agent::Base32,
    error::{SkunkError, SkunkResult},
    signature::Signature,
};
use lib3h_sodium::{
    pwhash::{ALG_ARGON2ID13, MEMLIMIT_INTERACTIVE, OPSLIMIT_INTERACTIVE},
    secbuf::SecBuf,
};
use serde::{self};
use serde::{Serialize, Deserialize};
use crate::{passphrase_manager::PassphraseManager};
use sx_dpki::{password_encryption::PwHashConfig, seed::SeedType};
use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::prelude::*,
    path::PathBuf,
    sync::Arc,
};

const PCHECK_HEADER_SIZE: usize = 8;
const PCHECK_HEADER: [u8; 8] = *b"PHCCHECK";
const PCHECK_RANDOM_SIZE: usize = 32;
const PCHECK_SIZE: usize = PCHECK_RANDOM_SIZE + PCHECK_HEADER_SIZE;
const KEYBUNDLE_SIGNKEY_SUFFIX: &str = ":sign_key";
const KEYBUNDLE_ENCKEY_SUFFIX: &str = ":enc_key";
pub const PRIMARY_KEYBUNDLE_ID: &str = "primary_keybundle";
pub const STANDALONE_ROOT_SEED: &str = "root_seed";

pub enum Secret {
    SigningKey(SigningKeyPair),
    EncryptingKey(EncryptingKeyPair),
    Seed(SecBuf),
}

pub enum KeyType {
    Signing,
    Encrypting,
}

/// A type for providing high-level crypto functions and managing secrets securely.
/// Keystore can store an arbitrary number of named secrets such as key pairs and seeds.
/// It can be serialized and deserialized with serde and stores secrets in encrypted [KeyBlob]s,
/// both in the serialized format as well as in memory, as long as secrets are not used.
/// Once a secret is requested, it gets decrypted and cached in the clear in secure memory
/// ([SecBuf]).
///
/// Passphrases for de-/encryption are requested from the [PassphraseManager] that has to be
/// provided on creation.
///
/// Keystore makes sure that all secrets are encrypted with the same passphrase, so although
/// every secrete is stored separately in its own [KeyBlob], the whole Keystore *has* a logical
/// passphrase. If a function that requires a passphrase receives a different passphrase from the
/// [PassphraseManager] as was used to create this keystore, it will fail.
///
/// It provides high-level functions for key/seed derivation such as:
/// * [add_seed_from_seed]
/// * [add_key_from_seed]
/// * [add_signing_key_from_seed]
/// * [add_encrypting_key_from_seed]
///
/// and a [sign] function for using stored keys to create signatures.
///
#[derive(Serialize, Deserialize)]
pub struct Keystore {
    /// This stores the cipher text of [PCHECK_HEADER] plus 32 random bytes encrypted
    /// with the keystore's passphrase.
    /// Any encryption will only happen after checking the provided passphrase to be
    /// able to decrypt this cipher and get back the [PCHECK_HEADER], as to make sure
    /// that every (separately) encrypted secret within this store is encrypted with
    /// the same passphrase.
    passphrase_check: String,

    /// These are the secrets (keys/seeds) stored encrypted, by name.
    secrets: BTreeMap<String, KeyBlob>,

    // The following fields are transient, i.e. not serialized to the keystore file:
    /// Using a secret from [secrets] will result in decrypting the secret and
    /// storing it in this cache.
    /// TODO: maybe clear the cache for certain (not agent keys) items after some time?
    #[serde(skip_serializing, skip_deserializing)]
    cache: HashMap<String, Arc<Mutex<Secret>>>,

    /// Requested for passphrases needed to decrypt secrets
    #[serde(skip_serializing, skip_deserializing)]
    passphrase_manager: Option<Arc<PassphraseManager>>,

    /// Hash config used for hashing passphrases.
    /// Gets sets to non-default for quick tests.
    #[serde(skip_serializing, skip_deserializing)]
    hash_config: Option<PwHashConfig>,
}

fn make_passphrase_check(
    passphrase: &mut SecBuf,
    hash_config: Option<PwHashConfig>,
) -> SkunkResult<String> {
    let mut check_buf = SecBuf::with_secure(PCHECK_SIZE);
    check_buf.randomize();
    check_buf.write(0, &PCHECK_HEADER).unwrap();
    encrypt_with_passphrase_buf(&mut check_buf, passphrase, hash_config)
}

// TODO: #[holochain_tracing_macros::newrelic_autotrace(HOLOCHAIN_CONDUCTOR_LIB)]
impl Keystore {
    /// Create a new keystore.
    /// This will query `passphrase_manager` immediately to set a passphrase for the keystore.
    pub fn new(
        passphrase_manager: Arc<PassphraseManager>,
        mut hash_config: Option<PwHashConfig>,
    ) -> SkunkResult<Self> {
        if hash_config.is_none() {
            hash_config = test_hash_config()
        }
        Ok(Keystore {
            passphrase_check: make_passphrase_check(
                &mut passphrase_manager.get_passphrase()?,
                hash_config.clone(),
            )?,
            secrets: BTreeMap::new(),
            cache: HashMap::new(),
            passphrase_manager: Some(passphrase_manager),
            hash_config,
        })
    }

    /// Create a new keystore for "standalone" use, i.e. not initialized by a DPKI instance
    pub fn new_standalone(
        passphrase_manager: Arc<PassphraseManager>,
        hash_config: Option<PwHashConfig>,
    ) -> SkunkResult<(Self, Base32)> {
        let mut keystore = Keystore::new(passphrase_manager, hash_config)?;
        keystore.add_random_seed(STANDALONE_ROOT_SEED, SEED_SIZE)?;
        let (pub_key, _) =
            keystore.add_keybundle_from_seed(STANDALONE_ROOT_SEED, PRIMARY_KEYBUNDLE_ID)?;
        Ok((keystore, pub_key))
    }

    /// Load a keystore from file.
    /// This won't ask for a passphrase until a secret is used via the other functions.
    /// Secrets will get loaded to memory instantly but stay encrypted until requested.
    pub fn new_from_file(
        path: PathBuf,
        passphrase_manager: Arc<PassphraseManager>,
        hash_config: Option<PwHashConfig>,
    ) -> SkunkResult<Self> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let mut keystore: Keystore = serde_json::from_str(&contents)?;
        keystore.hash_config = hash_config.or_else(|| test_hash_config());
        keystore.passphrase_manager = Some(passphrase_manager);
        Ok(keystore)
    }

    /// This tries to decrypt `passphrase_check` with the given passphrase and
    /// expects to read `PCHECK_HEADER` from the decrypted text, ignoring the
    /// random bytes following the header.
    fn check_passphrase(&self, mut passphrase: &mut SecBuf) -> SkunkResult<bool> {
        let mut decrypted_buf = decrypt_with_passphrase_buf(
            &self.passphrase_check,
            &mut passphrase,
            self.hash_config.clone(),
            PCHECK_SIZE,
        )?;
        let mut decrypted_header = SecBuf::with_insecure(PCHECK_HEADER_SIZE);
        let decrypted_buf = decrypted_buf.read_lock();
        decrypted_header.write(0, &decrypted_buf[0..PCHECK_HEADER_SIZE])?;
        let mut expected_header = SecBuf::with_secure(PCHECK_HEADER_SIZE);
        expected_header.write(0, &PCHECK_HEADER)?;
        Ok(decrypted_header.compare(&mut expected_header) == 0)
    }

    pub fn change_passphrase(
        &mut self,
        old_passphrase: &mut SecBuf,
        new_passphrase: &mut SecBuf,
    ) -> SkunkResult<()> {
        if !self.check_passphrase(old_passphrase)? {
            return Err(SkunkError::Todo("Bad passphrase".to_string()));
        }
        self.passphrase_check = make_passphrase_check(new_passphrase, self.hash_config.clone())?;
        Ok(())
    }

    /// Actually runs the decryption of the given KeyBlob with the given passphrase.
    /// Called by decrypt().
    /// Calls the matching from_blob function depending on the type of the KeyBlob.
    fn inner_decrypt(&self, blob: &KeyBlob, mut passphrase: SecBuf) -> Result<Secret, SkunkError> {
        Ok(match blob.blob_type {
            BlobType::Seed => {
                Secret::Seed(Seed::from_blob(blob, &mut passphrase, self.hash_config.clone())?.buf)
            }
            BlobType::SigningKey => Secret::SigningKey(SigningKeyPair::from_blob(
                blob,
                &mut passphrase,
                self.hash_config.clone(),
            )?),
            BlobType::EncryptingKey => Secret::EncryptingKey(EncryptingKeyPair::from_blob(
                blob,
                &mut passphrase,
                self.hash_config.clone(),
            )?),
            _ => {
                return Err(SkunkError::Todo(
                    "Tried to decrypt unsupported BlobType in Keystore: {}".to_string(),
                ));
            }
        })
    }

    /// This function expects the named secret in `secrets`, decrypts it and stores the decrypted
    /// representation in `cache`.
    fn decrypt(&mut self, id_str: &String) -> SkunkResult<()> {
        let blob = self.secrets.get(id_str).ok_or("Secret not found".to_string())?;

        let mut default_passphrase =
            SecBuf::with_insecure_from_string(holochain_common::DEFAULT_PASSPHRASE.to_string());

        let maybe_secret = if Ok(true) == self.check_passphrase(&mut default_passphrase) {
            self.inner_decrypt(blob, default_passphrase)
        } else {
            let passphrase = self.passphrase_manager.as_ref().ok_or(SkunkError::NoneError)?.get_passphrase()?;
            self.inner_decrypt(blob, passphrase)
        };

        let secret = maybe_secret.map_err(|err| {
            SkunkError::Todo(format!("Could not decrypt '{}': {:?}", id_str, err))
        })?;

        self.cache
            .insert(id_str.clone(), Arc::new(Mutex::new(secret)));
        Ok(())
    }

    /// This expects an unencrypted named secret in `cache`, encrypts it and stores the
    /// encrypted representation in `secrets`.
    fn encrypt(&mut self, id_str: &String) -> SkunkResult<()> {
        let secret = self.cache.get(id_str).ok_or("Secret not found".to_string())?;
        let mut passphrase = self.passphrase_manager.as_ref().ok_or(SkunkError::NoneError)?.get_passphrase()?;
        self.check_passphrase(&mut passphrase)?;
        let blob = match *secret.lock()? {
            Secret::Seed(ref mut buf) => {
                let mut owned_buf = SecBuf::with_insecure(buf.len());
                owned_buf.write(0, &*buf.read_lock())?;
                Seed::new(owned_buf, SeedType::OneShot).as_blob(
                    &mut passphrase,
                    "".to_string(),
                    self.hash_config.clone(),
                )
            }
            Secret::SigningKey(ref mut key) => {
                key.as_blob(&mut passphrase, "".to_string(), self.hash_config.clone())
            }
            Secret::EncryptingKey(ref mut key) => {
                key.as_blob(&mut passphrase, "".to_string(), self.hash_config.clone())
            }
        }?;
        self.secrets.insert(id_str.clone(), blob);
        Ok(())
    }

    /// Serialize the keystore to a file.
    pub fn save(&self, path: PathBuf) -> SkunkResult<()> {
        let json_string = serde_json::to_string(self)?;
        let mut file = File::create(path)?;
        file.write_all(&json_string.as_bytes())?;
        Ok(())
    }

    /// return a list of the identifiers stored in the keystore
    pub fn list(&self) -> Vec<String> {
        self.secrets.keys().map(|k| k.to_string()).collect()
    }

    /// adds a secret to the keystore
    pub fn add(&mut self, dst_id_str: &str, secret: Arc<Mutex<Secret>>) -> SkunkResult<()> {
        let dst_id = self.check_dst_identifier(dst_id_str)?;
        self.cache.insert(dst_id.clone(), secret);
        self.encrypt(&dst_id)?;
        Ok(())
    }

    /// adds a random root seed into the keystore
    pub fn add_random_seed(&mut self, dst_id_str: &str, size: usize) -> SkunkResult<()> {
        let seed_buf = generate_random_buf(size);
        let secret = Arc::new(Mutex::new(Secret::Seed(seed_buf)));
        self.add(dst_id_str, secret)
    }

    /// adds a provided root seed into the keystore
    pub fn add_seed(&mut self, dst_id_str: &str, seed: &[u8]) -> SkunkResult<()> {
        let mut seed_buf = SecBuf::with_secure(seed.len());
        seed_buf.from_array(seed)?;
        let secret = Arc::new(Mutex::new(Secret::Seed(seed_buf)));
        self.add(dst_id_str, secret)
    }

    fn check_dst_identifier(&self, dst_id_str: &str) -> SkunkResult<String> {
        let dst_id = dst_id_str.to_string();
        if self.secrets.contains_key(&dst_id) {
            return Err(SkunkError::Todo(
                "identifier already exists".to_string(),
            ));
        }
        Ok(dst_id)
    }

    /// gets a secret from the keystore
    pub fn get(&mut self, src_id_str: &str) -> SkunkResult<Arc<Mutex<Secret>>> {
        let src_id = src_id_str.to_string();
        if !self.secrets.contains_key(&src_id) {
            return Err(SkunkError::Todo(
                "unknown source identifier".to_string(),
            ));
        }

        if !self.cache.contains_key(&src_id) {
            self.decrypt(&src_id)?;
        }

        Ok(self.cache.get(&src_id).unwrap().clone()) // unwrap ok because we made sure src exists
    }

    fn check_identifiers(
        &mut self,
        src_id_str: &str,
        dst_id_str: &str,
    ) -> SkunkResult<(Arc<Mutex<Secret>>, String)> {
        let dst_id = self.check_dst_identifier(dst_id_str)?;
        let src_secret = self.get(src_id_str)?;
        Ok((src_secret, dst_id))
    }

    /// adds a derived seed into the keystore
    pub fn add_seed_from_seed(
        &mut self,
        src_id_str: &str,
        dst_id_str: &str,
        context: &SeedContext,
        index: u64,
    ) -> SkunkResult<()> {
        let (src_secret, dst_id) = self.check_identifiers(src_id_str, dst_id_str)?;
        let secret = {
            let mut src_secret = src_secret.lock().unwrap();
            match *src_secret {
                Secret::Seed(ref mut src) => {
                    let seed = generate_derived_seed_buf(src, context, index, SEED_SIZE)?;
                    Arc::new(Mutex::new(Secret::Seed(seed)))
                }
                _ => {
                    return Err(SkunkError::Todo(
                        "source secret is not a root seed".to_string(),
                    ));
                }
            }
        };
        self.cache.insert(dst_id.clone(), secret);
        self.encrypt(&dst_id)?;

        Ok(())
    }

    /// adds a keypair into the keystore based on a seed already in the keystore
    /// returns the public key
    pub fn add_key_from_seed(
        &mut self,
        src_id_str: &str,
        dst_id_str: &str,
        key_type: KeyType,
    ) -> SkunkResult<Base32> {
        let (src_secret, dst_id) = self.check_identifiers(src_id_str, dst_id_str)?;
        let (secret, public_key) = {
            let mut src_secret = src_secret.lock().unwrap();
            let ref mut seed_buf = match *src_secret {
                Secret::Seed(ref mut src) => src,
                _ => {
                    return Err(SkunkError::Todo(
                        "source secret is not a seed".to_string(),
                    ));
                }
            };
            match key_type {
                KeyType::Signing => {
                    let key_pair = SigningKeyPair::new_from_seed(seed_buf)?;
                    let public_key = key_pair.public();
                    (
                        Arc::new(Mutex::new(Secret::SigningKey(key_pair))),
                        public_key,
                    )
                }
                KeyType::Encrypting => {
                    let key_pair = EncryptingKeyPair::new_from_seed(seed_buf)?;
                    let public_key = key_pair.public();
                    (
                        Arc::new(Mutex::new(Secret::EncryptingKey(key_pair))),
                        public_key,
                    )
                }
            }
        };
        self.cache.insert(dst_id.clone(), secret);
        self.encrypt(&dst_id)?;

        Ok(public_key)
    }

    /// adds a signing keypair into the keystore based on a seed already in the keystore
    /// returns the public key
    pub fn add_signing_key_from_seed(
        &mut self,
        src_id_str: &str,
        dst_id_str: &str,
    ) -> SkunkResult<Base32> {
        self.add_key_from_seed(src_id_str, dst_id_str, KeyType::Signing)
    }

    /// adds an encrypting keypair into the keystore based on a seed already in the keystore
    /// returns the public key
    pub fn add_encrypting_key_from_seed(
        &mut self,
        src_id_str: &str,
        dst_id_str: &str,
    ) -> SkunkResult<Base32> {
        self.add_key_from_seed(src_id_str, dst_id_str, KeyType::Encrypting)
    }

    /// adds a keybundle into the keystore based on a seed already in the keystore by
    /// adding two keypair secrets (signing and encrypting) under the named prefix
    /// returns the public keys of the secrets
    pub fn add_keybundle_from_seed(
        &mut self,
        src_id_str: &str,
        dst_id_prefix_str: &str,
    ) -> SkunkResult<(Base32, Base32)> {
        let dst_sign_id_str = [dst_id_prefix_str, KEYBUNDLE_SIGNKEY_SUFFIX].join("");
        let dst_enc_id_str = [dst_id_prefix_str, KEYBUNDLE_ENCKEY_SUFFIX].join("");

        let sign_pub_key =
            self.add_key_from_seed(src_id_str, &dst_sign_id_str, KeyType::Signing)?;
        let enc_pub_key =
            self.add_key_from_seed(src_id_str, &dst_enc_id_str, KeyType::Encrypting)?;
        Ok((sign_pub_key, enc_pub_key))
    }

    /// adds a keybundle into the keystore based on an actual keybundle object by
    /// adding two keypair secrets (signing and encrypting) under the named prefix
    pub fn add_keybundle(
        &mut self,
        dst_id_prefix_str: &str,
        keybundle: &mut KeyBundle,
    ) -> SkunkResult<()> {
        let dst_sign_id_str = [dst_id_prefix_str, KEYBUNDLE_SIGNKEY_SUFFIX].join("");
        let dst_enc_id_str = [dst_id_prefix_str, KEYBUNDLE_ENCKEY_SUFFIX].join("");

        let sign_keypair = keybundle.sign_keys.new_from_self()?;
        let enc_keypair = keybundle.enc_keys.new_from_self()?;
        let sign_secret = Arc::new(Mutex::new(Secret::SigningKey(sign_keypair)));
        let enc_secret = Arc::new(Mutex::new(Secret::EncryptingKey(enc_keypair)));
        self.add(&dst_sign_id_str, sign_secret)?;
        self.add(&dst_enc_id_str, enc_secret)?;
        Ok(())
    }

    /// adds a keybundle into the keystore based on a seed already in the keystore by
    /// adding two keypair secrets (signing and encrypting) under the named prefix
    /// returns the public keys of the secrets
    pub fn get_keybundle(&mut self, src_id_prefix_str: &str) -> SkunkResult<KeyBundle> {
        let src_sign_id_str = [src_id_prefix_str, KEYBUNDLE_SIGNKEY_SUFFIX].join("");
        let src_enc_id_str = [src_id_prefix_str, KEYBUNDLE_ENCKEY_SUFFIX].join("");

        let sign_secret = self.get(&src_sign_id_str)?;
        let mut sign_secret = sign_secret.lock().unwrap();
        let sign_key = match *sign_secret {
            Secret::SigningKey(ref mut key_pair) => key_pair.new_from_self()?,
            _ => {
                return Err(SkunkError::Todo(
                    "source secret is not a signing key".to_string(),
                ));
            }
        };

        let enc_secret = self.get(&src_enc_id_str)?;
        let mut enc_secret = enc_secret.lock().unwrap();
        let enc_key = match *enc_secret {
            Secret::EncryptingKey(ref mut key_pair) => key_pair.new_from_self()?,
            _ => {
                return Err(SkunkError::Todo(
                    "source secret is not an encrypting key".to_string(),
                ));
            }
        };

        Ok(KeyBundle::new(sign_key, enc_key)?)
    }

    /// signs some data using a keypair in the keystore
    /// returns the signature
    pub fn sign(&mut self, src_id_str: &str, data: String) -> SkunkResult<Signature> {
        let src_secret = self.get(src_id_str)?;
        let mut src_secret = src_secret.lock().unwrap();
        match *src_secret {
            Secret::SigningKey(ref mut key_pair) => {
                let mut data_buf = SecBuf::with_insecure_from_string(data);

                let mut signature_buf = key_pair.sign(&mut data_buf)?;
                let buf = signature_buf.read_lock();
                // Return as base64 encoded string
                let signature_str = base64::encode(&**buf);
                Ok(Signature::from(signature_str))
            }
            _ => {
                return Err(SkunkError::Todo(
                    "source secret is not a signing key".to_string(),
                ));
            }
        }
    }
}

pub fn test_hash_config() -> Option<PwHashConfig> {
    Some(PwHashConfig(
        OPSLIMIT_INTERACTIVE,
        MEMLIMIT_INTERACTIVE,
        ALG_ARGON2ID13,
    ))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::passphrase_manager::PassphraseServiceMock;
    use base64;
    use sx_dpki::utils;
    use sx_types::prelude::Address;

    fn mock_passphrase_manager(passphrase: String) -> Arc<PassphraseManager> {
        Arc::new(PassphraseManager::new(Arc::new(Mutex::new(
            PassphraseServiceMock { passphrase },
        ))))
    }

    fn new_test_keystore(passphrase: String) -> Keystore {
        Keystore::new(mock_passphrase_manager(passphrase), test_hash_config()).unwrap()
    }

    fn random_test_passphrase() -> String {
        let mut buf = utils::generate_random_buf(10);
        let read_lock = buf.read_lock();
        String::from_utf8_lossy(&*read_lock).to_string()
    }

    #[test]
    fn test_keystore_new() {
        let random_passphrase = random_test_passphrase();
        let keystore = new_test_keystore(random_passphrase.clone());
        let mut random_passphrase = SecBuf::with_insecure_from_string(random_passphrase);
        assert!(keystore.list().is_empty());
        assert_eq!(keystore.check_passphrase(&mut random_passphrase), Ok(true));
        let mut another_random_passphrase = utils::generate_random_buf(10);
        assert_eq!(
            keystore.check_passphrase(&mut another_random_passphrase),
            Ok(false)
        );
    }

    #[test]
    fn test_save_load_roundtrip() {
        let random_passphrase = random_test_passphrase();
        let mut keystore = new_test_keystore(random_passphrase.clone());
        let zero = [
            0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0,
        ];
        assert_eq!(keystore.add_seed("my_zero_seed", &zero), Ok(()));
        assert_eq!(keystore.list(), vec!["my_zero_seed".to_string()]);
        assert_eq!(keystore.add_random_seed("my_root_seed", SEED_SIZE), Ok(()));
        assert_eq!(
            keystore.list(),
            vec!["my_root_seed".to_string(), "my_zero_seed".to_string()]
        );

        let mut path = PathBuf::new();
        path.push("tmp-test/test-keystore");
        keystore.save(path.clone()).unwrap();

        let mut loaded_keystore = Keystore::new_from_file(
            path.clone(),
            mock_passphrase_manager(random_passphrase),
            test_hash_config(),
        )
        .unwrap();
        assert_eq!(
            loaded_keystore.list(),
            vec!["my_root_seed".to_string(), "my_zero_seed".to_string()]
        );

        let secret1 = keystore.get("my_root_seed").unwrap();
        let expected_seed = match *secret1.lock().unwrap() {
            Secret::Seed(ref mut buf) => {
                let lock = buf.read_lock();
                String::from_utf8_lossy(&**lock).to_string()
            }
            _ => unreachable!(),
        };

        let secret2 = loaded_keystore.get("my_root_seed").unwrap();
        let loaded_seed = match *secret2.lock().unwrap() {
            Secret::Seed(ref mut buf) => {
                let lock = buf.read_lock();
                String::from_utf8_lossy(&**lock).to_string()
            }
            _ => unreachable!(),
        };

        assert_eq!(expected_seed, loaded_seed);

        let secret_zero = loaded_keystore.get("my_zero_seed").unwrap();
        let loaded_zero = match *secret_zero.lock().unwrap() {
            Secret::Seed(ref mut buf) => {
                let lock = buf.read_lock();
                (&**lock).to_owned()
            }
            _ => unreachable!(),
        };
        assert_eq!(&zero, &loaded_zero[..])
    }

    #[test]
    fn test_keystore_change_passphrase() {
        let random_passphrase = random_test_passphrase();
        let mut keystore = new_test_keystore(random_passphrase.clone());
        let mut random_passphrase = SecBuf::with_insecure_from_string(random_passphrase);
        let mut another_random_passphrase = utils::generate_random_buf(10);
        assert!(
            // wrong passphrase
            keystore
                .change_passphrase(&mut another_random_passphrase, &mut random_passphrase)
                .is_err()
        );
        assert_eq!(
            keystore.change_passphrase(&mut random_passphrase, &mut another_random_passphrase),
            Ok(())
        );
        // check that passphrase was actually changed
        assert_eq!(keystore.check_passphrase(&mut random_passphrase), Ok(false));
        assert_eq!(
            keystore.check_passphrase(&mut another_random_passphrase),
            Ok(true)
        );
    }

    #[test]
    fn test_keystore_add_random_seed() {
        let mut keystore = new_test_keystore(random_test_passphrase());

        assert_eq!(keystore.add_random_seed("my_root_seed", SEED_SIZE), Ok(()));
        assert_eq!(keystore.list(), vec!["my_root_seed".to_string()]);
        assert_eq!(
            keystore.add_random_seed("my_root_seed", SEED_SIZE),
            Err(SkunkError::Todo(
                "identifier already exists".to_string()
            ))
        );
    }

    #[test]
    fn test_keystore_add_seed_from_seed() {
        let mut keystore = new_test_keystore(random_test_passphrase());

        let context = SeedContext::new(*b"SOMECTXT");

        assert_eq!(
            keystore.add_seed_from_seed("my_root_seed", "my_second_seed", &context, 1),
            Err(SkunkError::Todo(
                "unknown source identifier".to_string()
            ))
        );

        let _ = keystore.add_random_seed("my_root_seed", SEED_SIZE);

        assert_eq!(
            keystore.add_seed_from_seed("my_root_seed", "my_second_seed", &context, 1),
            Ok(())
        );

        assert!(keystore.list().contains(&"my_root_seed".to_string()));
        assert!(keystore.list().contains(&"my_second_seed".to_string()));

        assert_eq!(
            keystore.add_seed_from_seed("my_root_seed", "my_second_seed", &context, 1),
            Err(SkunkError::Todo(
                "identifier already exists".to_string()
            ))
        );
    }

    #[test]
    fn test_keystore_add_signing_key_from_seed() {
        let mut keystore = new_test_keystore(random_test_passphrase());

        assert_eq!(
            keystore.add_signing_key_from_seed("my_root_seed", "my_keypair"),
            Err(SkunkError::Todo(
                "unknown source identifier".to_string()
            ))
        );

        let _ = keystore.add_random_seed("my_root_seed", SEED_SIZE);

        let result = keystore.add_signing_key_from_seed("my_root_seed", "my_keypair");
        assert!(!result.is_err());
        let pubkey = result.unwrap();
        assert!(format!("{}", pubkey).starts_with("Hc"));

        assert_eq!(
            keystore.add_signing_key_from_seed("my_root_seed", "my_keypair"),
            Err(SkunkError::Todo(
                "identifier already exists".to_string()
            ))
        );
    }

    #[test]
    fn test_keystore_sign() {
        let mut keystore = new_test_keystore(random_test_passphrase());
        let _ = keystore.add_random_seed("my_root_seed", SEED_SIZE);

        let data = base64::encode("the data to sign");

        assert_eq!(
            keystore.sign("my_keypair", data.clone()),
            Err(SkunkError::Todo(
                "unknown source identifier".to_string()
            ))
        );

        let public_key = keystore
            .add_signing_key_from_seed("my_root_seed", "my_keypair")
            .unwrap();

        let result = keystore.sign("my_keypair", data.clone());
        assert!(!result.is_err());

        let signature = result.unwrap();
        assert_eq!(String::from(signature.clone()).len(), 88); //88 is the size of a base64ized signature buf

        let result = utils::verify(Address::from(public_key), data.clone(), signature);
        assert!(!result.is_err());
        assert!(result.unwrap());

        keystore
            .add_encrypting_key_from_seed("my_root_seed", "my_enc_keypair")
            .unwrap();
        assert_eq!(
            keystore.sign("my_enc_keypair", data.clone()),
            Err(SkunkError::Todo(
                "source secret is not a signing key".to_string()
            ))
        );
    }

    #[test]
    fn test_keystore_keybundle() {
        let mut keystore = new_test_keystore(random_test_passphrase());

        assert_eq!(
            keystore.add_keybundle_from_seed("my_root_seed", "my_keybundle"),
            Err(SkunkError::Todo(
                "unknown source identifier".to_string()
            ))
        );

        let _ = keystore.add_random_seed("my_root_seed", SEED_SIZE);

        let result = keystore.add_keybundle_from_seed("my_root_seed", "my_keybundle");
        assert!(!result.is_err());
        let (sign_pubkey, enc_pubkey) = result.unwrap();
        assert!(format!("{}", sign_pubkey).starts_with("Hc"));

        assert_eq!(
            keystore.add_keybundle_from_seed("my_root_seed", "my_keybundle"),
            Err(SkunkError::Todo(
                "identifier already exists".to_string()
            ))
        );

        let result = keystore.get_keybundle("my_keybundle");
        assert!(!result.is_err());
        let mut key_bundle = result.unwrap();

        assert_eq!(key_bundle.sign_keys.public(), sign_pubkey);
        assert_eq!(key_bundle.enc_keys.public(), enc_pubkey);

        let result = keystore.add_keybundle("copy_of_keybundle", &mut key_bundle);
        assert!(!result.is_err());

        let result = keystore.get_keybundle("copy_of_keybundle");
        assert!(!result.is_err());

        let mut key_bundle_copy = result.unwrap();

        assert!(key_bundle.sign_keys.is_same(&mut key_bundle_copy.sign_keys));
        assert!(key_bundle.enc_keys.is_same(&mut key_bundle_copy.enc_keys));
    }

    #[test]
    /// Tests if the keystore encrypted with holochain_common::DEFAULT_PASSPHRASE can be decrypted,
    /// no matter what passphrase we get from the passphrase manager
    /// ("definitely wrong passphrase" should not be used at all since the default passphrase should
    /// be tried before even asking the passphrase manager)
    fn test_keystore_default_passphrase() {
        let mut loaded_keystore = Keystore::new_from_file(
            PathBuf::from("test_keystore"),
            mock_passphrase_manager("definitely wrong passphrase".to_string()),
            None,
        )
        .unwrap();
        assert_eq!(
            loaded_keystore.list(),
            vec![
                "primary_keybundle:enc_key",
                "primary_keybundle:sign_key",
                "root_seed"
            ]
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
        );

        let result = loaded_keystore.get_keybundle("primary_keybundle");

        assert!(!result.is_err());
        let mut key_bundle = result.unwrap();

        assert_eq!(
            key_bundle.sign_keys.public(),
            "HcSCIcOIYdE5spsmimrFfD9um6Pe7p9piu6g36TsaP55Us4docRdyj4dAnmbaui"
        );
        assert_eq!(
            key_bundle.enc_keys.public(),
            "HcKciaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );

        assert_eq!(base64::encode(&**key_bundle.sign_keys.private().read_lock()), "4qBbA4Bs+5Z7GLrOY67lUEtr5PX8MnPzhFwGZsKrUn4JqLjJuLorQuBSj/NfHE677kT4bPJRA7e5x0NooDunQw==".to_string());
        assert_eq!(
            base64::encode(&**key_bundle.enc_keys.private().read_lock()),
            "VX4j1zRvIT7FojcTsqJJfu81NU1bUgiKxqWZOl/bCR4=".to_string()
        );
    }
}
