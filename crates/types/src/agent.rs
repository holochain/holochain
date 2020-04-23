//! agent module

use holochain_serialized_bytes::prelude::*;

/// Agent Error Type
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    /// Crypto Error
    #[error("CryptoError: {0}")]
    CryptoError(#[from] holochain_crypto::CryptoError),

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
struct AgentPrivateKey(holochain_crypto::DynCryptoBytes);

/// This struct represents a fully realized agent able to sign things.
pub struct Agent {
    private_key: AgentPrivateKey,
    public_key: holo_hash::AgentHash,
}

impl Agent {
    /// Create an agent keypair from pure entropy (no seed)
    pub async fn from_pure_entropy() -> AgentResult<Self> {
        let (public_key, private_key) = holochain_crypto::crypto_sign_keypair(None).await?;
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
        let mut data = holochain_crypto::crypto_insecure_buffer_from_bytes(data)?;
        let signature = holochain_crypto::crypto_sign(&mut data, &mut self.private_key.0).await?;
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
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
            holochain_crypto::DynCryptoBytes,
        )> = (|| {
            let pub_key = holochain_crypto::crypto_insecure_buffer_from_bytes(self.get_bytes())?;
            let signature = holochain_crypto::crypto_insecure_buffer_from_bytes(&signature.0)?;
            let data = holochain_crypto::crypto_insecure_buffer_from_bytes(data)?;
            Ok((signature, data, pub_key))
        })();

        async move {
            let (mut signature, mut data, mut pub_key) = result?;
            Ok(
                holochain_crypto::crypto_sign_verify(&mut signature, &mut data, &mut pub_key)
                    .await?,
            )
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
            let _ = holochain_crypto::crypto_init_sodium();

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
