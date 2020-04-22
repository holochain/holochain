//! agent module

/// Agent Error Type
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Crypto Error
    #[error("CryptoError: {0}")]
    CryptoError(#[from] sx_crypto::CryptoError),

    /// Holo Hash Error
    #[error("HoloHashError: {0}")]
    HoloHashError(#[from] holo_hash::HoloHashError),
}

/// Agent Result Type
pub type AgentResult<T> = Result<T, AgentError>;

/// A signature produced by an Agent private key
#[derive(Serialize, Deserialize, SerializedBytes)]
pub struct AgentSignature(#[serde(with = "serde_bytes")] Vec<u8>);

/// this struct represents the raw private key bytes of an agent signing keypair
/// Note - this is the part we'd like to extract into a different "keystore"
/// system service so we don't expose the private keys as much.
struct AgentPrivateKey(sx_crypto::DynCryptoBytes);

/// This struct represents a fully realized agent able to sign things.
pub struct Agent {
    private_key: AgentPrivateKey,
    public_key: holo_hash::AgentHash,
}

impl Agent {
    /// Create an agent keypair from pure entropy (no seed)
    pub async fn from_pure_entropy() -> AgentResult<Self> {
        let (public_key, private_key) = sx_crypto::crypto_sign_keypair(None).await?;
        let public_key = public_key.read().to_vec();
        Ok(Self {
            private_key: AgentPrivateKey(private_key),
            public_key: holo_hash::AgentHash::with_pre_hashed(public_key).await,
        })
    }

    /// Retrieve the AgentHash / Public Key for this Agent
    pub fn agent_hash(&self) -> &holo_hash::AgentHash {
        &self.public_key
    }

    /// Sign some arbitrary data
    pub async fn sign(&mut self, data: &[u8]) -> AgentResult<AgentSignature> {
        let mut data = sx_crypto::crypto_insecure_buffer_from_bytes(data)?;
        let signature = sx_crypto::crypto_sign(&mut data, &mut self.private_key.0).await?;
        let signature = signature.read().to_vec();
        Ok(AgentSignature(signature))
    }
}

/// add signature verification functionality to AgentHash's
/// (because they are actually public keys)
pub trait AgentHashExt {
    /// verify a signature for given data with this agent public_key is valid
    fn verify_signature(
        &self,
        signature: &AgentSignature,
        data: &[u8],
    ) -> must_future::MustBoxFuture<'static, AgentResult<bool>>;
}

impl AgentHashExt for holo_hash::AgentHash {
    fn verify_signature(
        &self,
        signature: &AgentSignature,
        data: &[u8],
    ) -> must_future::MustBoxFuture<'static, AgentResult<bool>> {
        use futures::future::FutureExt;
        use holo_hash::HoloHashCoreHash;

        let result: AgentResult<(
            sx_crypto::DynCryptoBytes,
            sx_crypto::DynCryptoBytes,
            sx_crypto::DynCryptoBytes,
        )> = (|| {
            let pub_key = sx_crypto::crypto_insecure_buffer_from_bytes(self.get_bytes())?;
            let signature = sx_crypto::crypto_insecure_buffer_from_bytes(&signature.0)?;
            let data = sx_crypto::crypto_insecure_buffer_from_bytes(data)?;
            Ok((signature, data, pub_key))
        })();

        async move {
            let (mut signature, mut data, mut pub_key) = result?;
            Ok(sx_crypto::crypto_sign_verify(&mut signature, &mut data, &mut pub_key).await?)
        }
        .boxed()
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn test_agent_signature_sanity() {
        tokio::task::spawn(async move {
            let _ = sx_crypto::crypto_init_sodium();

            let mut agent1 = Agent::from_pure_entropy().await.unwrap();
            let agent1_hash = agent1.agent_hash().clone();

            let agent2 = Agent::from_pure_entropy().await.unwrap();
            let agent2_hash = agent2.agent_hash().clone();

            let test_data1 = b"yadayada";
            let test_data2 = b"yadayada2";

            let signature = agent1.sign(test_data1).await.unwrap();

            assert!(agent1_hash
                .verify_signature(&signature, test_data1)
                .await
                .unwrap());
            assert!(!agent1_hash
                .verify_signature(&signature, test_data2)
                .await
                .unwrap());
            assert!(!agent2_hash
                .verify_signature(&signature, test_data1)
                .await
                .unwrap());
        })
        .await
        .unwrap();
    }
}

// TODO XXX |------------------------------------------------------------------|
// TODO XXX | Everything below here is OLD stuff from holochain-rust - DELETE! |
// TODO XXX |------------------------------------------------------------------|

use crate::persistence::cas::content::{Address, Addressable};
use hcid::*;
use holochain_serialized_bytes::prelude::*;
use std::str;

/// Base32...as a String?
pub type Base32 = String;

/// AgentId represents an agent in the Holochain framework.
/// This data struct is meant be stored in the CAS and source-chain.
/// Its key is the public signing key, and is also used as its address.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, SerializedBytes, Eq, Hash)]
pub struct AgentId {
    /// a nickname for referencing this agent
    nick: String,
    /// the encoded public signing key of this agent (the magnifier)
    pub_sign_key: Base32,
    // TODO: Add the encoded public encrypting key (the safe / padlock)
    // pub pub_enc_key: Base32,
}

impl AgentId {
    /// A well-known key useful for testing and used by generate_fake()
    pub const FAKE_RAW_KEY: [u8; 32] = [
        42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    /// generate an agent id with fake key
    pub fn generate_fake(nick: &str) -> Self {
        AgentId::new_with_raw_key(nick, str::from_utf8(&AgentId::FAKE_RAW_KEY).unwrap())
            .expect("AgentId fake key generation failed")
    }

    /// initialize an Agent struct with `nick` and `key` that will be encoded with HCID.
    pub fn new_with_raw_key(nick: &str, key: &str) -> Result<Self, HcidError> {
        let codec = HcidEncoding::with_kind("hcs0")?;
        let key_b32 = codec.encode(key.as_bytes())?;
        Ok(AgentId::new(nick, key_b32))
    }

    /// initialize an Agent struct with `nick` and a HCID encoded key.
    pub fn new(nick: &str, key_b32: Base32) -> Self {
        AgentId {
            nick: nick.to_string(),
            pub_sign_key: key_b32,
        }
    }

    /// Get the key decoded with HCID
    pub fn decoded_key(&self) -> Result<String, HcidError> {
        let codec = HcidEncoding::with_kind("hcs0")?;
        let key_b32 = codec.decode(&self.pub_sign_key)?;
        Ok(str::from_utf8(&key_b32).unwrap().to_owned())
    }

    /// Agent nick-name
    pub fn nick(&self) -> &String {
        &self.nick
    }

    /// public signing key
    pub fn pub_sign_key(&self) -> &Base32 {
        &self.pub_sign_key
    }
}

impl Addressable for AgentId {
    /// for an Agent, the address is their public base32 encoded public signing key string
    fn address(&self) -> Address {
        self.pub_sign_key.clone().into()
    }
}

// should these not be in the tests module?!?

/// Valid test agent id
pub static GOOD_ID: &str = "HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa";
/// Invalid test agent id
pub static BAD_ID: &str = "HcScIkRaAaaaaaaaaaAaaaBBBBaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa";
/// Invalid test agent id #2
pub static TOO_BAD_ID: &str = "HcScIkRaAaaaaaaaaaBBBBBBBBaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa";

/// get a valid test agent id
pub fn test_agent_id() -> AgentId {
    AgentId::new("bob", GOOD_ID.to_string())
}

/// get a named test agent id
pub fn test_agent_id_with_name(name: &str) -> AgentId {
    AgentId::new(name, name.to_string())
}

#[cfg(test)]
mod old_tests {
    use super::*;

    pub fn test_identity_value() -> SerializedBytes {
        SerializedBytes::try_from(AgentId {
            nick: "bob".to_string(),
            pub_sign_key: GOOD_ID.to_string(),
        })
        .unwrap()
    }

    #[test]
    fn it_can_generate_fake() {
        let agent_id = AgentId::generate_fake("sandwich");
        assert_eq!(
            "HcScIkRaAaaaaaaaaaAaaaAAAAaaaaaaaaAaaaaAaaaaaaaaAaaAAAAatzu4aqa".to_string(),
            agent_id.address().to_string(),
        );
    }

    #[test]
    fn it_should_decode_key() {
        let agent_id = test_agent_id();
        let raw_key = agent_id.decoded_key().unwrap();
        println!("decoded key = {}", raw_key);
    }

    #[test]
    fn it_should_correct_errors() {
        let corrected_id = AgentId::new("bob", BAD_ID.to_string());
        let raw_key = corrected_id.decoded_key().unwrap();
        assert_eq!(test_agent_id().decoded_key().unwrap(), raw_key);
    }

    #[test]
    fn it_fails_if_too_many_errors() {
        let corrected_id = AgentId::new("bob", TOO_BAD_ID.to_string());
        let maybe_key = corrected_id.decoded_key();
        assert!(maybe_key.is_err());
    }

    #[test]
    /// show ToString implementation for Agent
    fn agent_to_string_test() {
        assert_eq!(test_identity_value(), test_agent_id().try_into().unwrap());
    }
}
