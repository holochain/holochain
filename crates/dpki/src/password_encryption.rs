use crate::NEW_RELIC_LICENSE_KEY;
pub use aead::{ABYTES, NONCEBYTES};
use sx_types::error::SkunkResult;
use lib3h_sodium::{aead, kx, pwhash, secbuf::SecBuf};
pub use pwhash::SALTBYTES;
use serde::{Deserialize, Serialize};

pub type OpsLimit = u64;
pub type MemLimit = usize;
pub type PwHashAlgo = i8;

#[derive(Clone)]
pub struct PwHashConfig(pub OpsLimit, pub MemLimit, pub PwHashAlgo);

/// Struct holding the result of a passphrase encryption
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EncryptedData {
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub cipher: Vec<u8>,
}

/// Simple API for generating a password hash with our set parameters
/// @param {SecBuf} password - the password buffer to hash
/// @param {SecBuf} salt - if specified, hash with this salt (otherwise random)
/// @param {SecBuf} hash_result - Empty SecBuf to receive the resulting hash.
/// @param {Option<PwHashConfig>} config - Optional hashing settings
/// TODO make salt optional
// TODO: #[holochain_tracing_macros::newrelic_autotrace(sx_dpki)]
pub(crate) fn pw_hash(
    password: &mut SecBuf,
    salt: &mut SecBuf,
    hash_result: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> SkunkResult<()> {
    let config = config.unwrap_or(PwHashConfig(
        pwhash::OPSLIMIT_SENSITIVE,
        pwhash::MEMLIMIT_SENSITIVE,
        pwhash::ALG_ARGON2ID13,
    ));
    pwhash::hash(password, config.0, config.1, config.2, salt, hash_result)?;
    Ok(())
}

/// Simple API for encrypting a buffer with a pwhash-ed passphrase
/// @param {Buffer} data - the data to encrypt
/// @param {SecBuf} passphrase - the passphrase to use for encrypting
/// @param {Option<PwHashConfig>} config - Optional encrypting settings
/// @return {EncryptedData} - the resulting encrypted data
// TODO: #[holochain_tracing_macros::newrelic_autotrace(sx_dpki)]
pub(crate) fn pw_enc(
    data: &mut SecBuf,
    passphrase: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> SkunkResult<EncryptedData> {
    let mut salt = SecBuf::with_insecure(SALTBYTES);
    salt.randomize();
    let mut nonce = SecBuf::with_insecure(NONCEBYTES);
    nonce.randomize();
    pw_enc_base(data, passphrase, &mut salt, &mut nonce, config)
}

/// Simple API for encrypting a buffer with a pwhash-ed passphrase but uses a zero nonce
/// This does not weaken security provided the same passphrase/salt is not used to encrypt multiple
/// pieces of data. Since a random salt is produced by this function it should not be an issue.
///  Helpful for reducing the size of the output EncryptedData (by NONCEBYTES)
/// @param {Buffer} data - the data to encrypt
/// @param {SecBuf} passphrase - the passphrase to use for encrypting
/// @param {Option<PwHashConfig>} config - Optional encrypting settings
/// @return {EncryptedData} - the resulting encrypted data
// TODO: #[holochain_tracing_macros::newrelic_autotrace(sx_dpki)]
pub(crate) fn pw_enc_zero_nonce(
    data: &mut SecBuf,
    passphrase: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> SkunkResult<EncryptedData> {
    let mut salt = SecBuf::with_insecure(SALTBYTES);
    salt.randomize();
    let mut nonce = SecBuf::with_insecure(NONCEBYTES);
    nonce.write(0, &[0; NONCEBYTES])?;
    let data = pw_enc_base(data, passphrase, &mut salt, &mut nonce, config)?;
    Ok(data)
}

/// Private general wrapper of pw_enc
// TODO: #[holochain_tracing_macros::newrelic_autotrace(sx_dpki)]
fn pw_enc_base(
    data: &mut SecBuf,
    passphrase: &mut SecBuf,
    mut salt: &mut SecBuf,
    mut nonce: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> SkunkResult<EncryptedData> {
    let mut secret = SecBuf::with_secure(kx::SESSIONKEYBYTES);
    let mut cipher = SecBuf::with_insecure(data.len() + aead::ABYTES);
    pw_hash(passphrase, &mut salt, &mut secret, config)?;
    aead::enc(data, &mut secret, None, &mut nonce, &mut cipher)?;

    let salt = salt.read_lock().to_vec();
    let nonce = nonce.read_lock().to_vec();
    let cipher = cipher.read_lock().to_vec();
    // Done
    Ok(EncryptedData {
        salt,
        nonce,
        cipher,
    })
}

/// Simple API for decrypting a buffer with a pwhash-ed passphrase
/// @param {EncryptedData} encrypted_data - the data to decrypt
/// @param {SecBuf} passphrase - the passphrase to use for encrypting
/// @param {SecBuf} decrypted_data - the dresulting ecrypted data
/// @param {Option<PwHashConfig>} config - Optional decrypting settings
// TODO: #[holochain_tracing_macros::newrelic_autotrace(sx_dpki)]
pub(crate) fn pw_dec(
    encrypted_data: &EncryptedData,
    passphrase: &mut SecBuf,
    decrypted_data: &mut SecBuf,
    config: Option<PwHashConfig>,
) -> SkunkResult<()> {
    let mut secret = SecBuf::with_secure(kx::SESSIONKEYBYTES);
    let mut salt = SecBuf::with_insecure(SALTBYTES);
    salt.from_array(&encrypted_data.salt)
        .expect("Failed to write SecBuf with array");
    let mut nonce = SecBuf::with_insecure(encrypted_data.nonce.len());
    nonce
        .from_array(&encrypted_data.nonce)
        .expect("Failed to write SecBuf with array");
    let mut cipher = SecBuf::with_insecure(encrypted_data.cipher.len());
    cipher
        .from_array(&encrypted_data.cipher)
        .expect("Failed to write SecBuf with array");
    pw_hash(passphrase, &mut salt, &mut secret, config)?;
    aead::dec(decrypted_data, &mut secret, None, &mut nonce, &mut cipher)?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub const TEST_CONFIG: Option<PwHashConfig> = Some(PwHashConfig(
        pwhash::OPSLIMIT_INTERACTIVE,
        pwhash::MEMLIMIT_INTERACTIVE,
        pwhash::ALG_ARGON2ID13,
    ));

    fn test_password() -> SecBuf {
        let mut password = SecBuf::with_insecure(pwhash::HASHBYTES);
        {
            let mut password = password.write_lock();
            password[0] = 42;
            password[1] = 222;
        }
        password
    }

    #[test]
    fn it_should_encrypt_data() {
        let mut password = test_password();
        let mut data = SecBuf::with_insecure(32);
        {
            let mut data = data.write_lock();
            data[0] = 88;
            data[1] = 101;
        }
        let encrypted_data = pw_enc(&mut data, &mut password, TEST_CONFIG).unwrap();

        let mut decrypted_data = SecBuf::with_insecure(32);
        pw_dec(
            &encrypted_data,
            &mut password,
            &mut decrypted_data,
            TEST_CONFIG,
        )
        .unwrap();

        let data = data.read_lock();
        let decrypted_data = decrypted_data.read_lock();
        assert_eq!(format!("{:?}", *decrypted_data), format!("{:?}", *data));
    }

    #[test]
    fn it_should_generate_pw_hash_with_salt() {
        let mut password = test_password();
        let mut salt = SecBuf::with_insecure(SALTBYTES);
        let mut hashed_password = SecBuf::with_insecure(pwhash::HASHBYTES);
        pw_hash(&mut password, &mut salt, &mut hashed_password, TEST_CONFIG).unwrap();
        println!("salt = {:?}", salt);
        {
            let pw2_hash = hashed_password.read_lock();
            assert_eq!(
                "[134, 156, 170, 171, 184, 19, 40, 158, 64, 227, 105, 252, 59, 175, 119, 226, 77, 238, 49, 61, 27, 174, 47, 246, 179, 168, 88, 200, 65, 11, 14, 159]",
                format!("{:?}", *pw2_hash),
            );
        }
        // hash with different salt should have different result
        salt.randomize();
        let mut hashed_password_b = SecBuf::with_insecure(pwhash::HASHBYTES);
        pw_hash(
            &mut password,
            &mut salt,
            &mut hashed_password_b,
            TEST_CONFIG,
        )
        .unwrap();
        assert!(hashed_password.compare(&mut hashed_password_b) != 0);

        // same hash should have same result
        let mut hashed_password_c = SecBuf::with_insecure(pwhash::HASHBYTES);
        pw_hash(
            &mut password,
            &mut salt,
            &mut hashed_password_c,
            TEST_CONFIG,
        )
        .unwrap();
        assert!(hashed_password_c.compare(&mut hashed_password_b) == 0);
    }
}
