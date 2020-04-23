//! DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!

use crate::*;
use ghost_actor::dependencies::futures::future::FutureExt;
use holochain_crypto::*;
use std::collections::HashMap;

/// DANGER! These Mock Keypairs should NEVER be used in production
/// The private keys have not been handled securely!
pub struct MockKeypair {
    /// The agent public key.
    pub pub_key: holo_hash::AgentHash,

    /// The private secret key DANGER - this is not handled securely!!
    pub sec_key: Vec<u8>,
}

/// Construct a new MockKeystore.
/// DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!
pub async fn spawn_mock_keystore(
    fixture_keypairs: Vec<MockKeypair>,
) -> Result<(KeystoreSender<()>, ghost_actor::GhostActorDriver), KeystoreError> {
    KeystoreSender::ghost_actor_spawn(Box::new(move |internal_sender| {
        async move { Ok(MockKeystore::new(internal_sender, fixture_keypairs)) }
            .boxed()
            .into()
    }))
    .await
}

/// Internal Private Key newtype.
#[derive(Debug)]
struct PrivateKey(pub holochain_crypto::DynCryptoBytes);

ghost_actor::ghost_chan! {
    name: MockKeystoreInternal,
    error: KeystoreError,
    api: {
        FinalizeNewKeypair::finalize_new_keypair (
            "we have generated a keypair, now track it",
            (holo_hash::AgentHash, PrivateKey),
            ()
        )
    }
}

/// Internal mock keystore struct.
struct MockKeystore {
    internal_sender: KeystoreInternalSender<(), MockKeystoreInternal>,
    fixture_keypairs: Vec<MockKeypair>,
    active_keypairs: HashMap<holo_hash::AgentHash, PrivateKey>,
}

impl MockKeystore {
    fn new(
        internal_sender: KeystoreInternalSender<(), MockKeystoreInternal>,
        fixture_keypairs: Vec<MockKeypair>,
    ) -> Self {
        Self {
            internal_sender,
            fixture_keypairs,
            active_keypairs: HashMap::new(),
        }
    }
}

impl KeystoreHandler<(), MockKeystoreInternal> for MockKeystore {
    fn handle_generate_sign_keypair_from_pure_entropy(
        &mut self,
    ) -> Result<KeystoreFuture<holo_hash::AgentHash>, KeystoreError> {
        if !self.fixture_keypairs.is_empty() {
            let MockKeypair { pub_key, sec_key } = self.fixture_keypairs.remove(0);
            // we're loading this out of insecure memory - but this is just a mock
            let sec_key = PrivateKey(danger_crypto_secure_buffer_from_bytes(&sec_key)?);
            self.active_keypairs.insert(pub_key.clone(), sec_key);
            return Ok(async move { Ok(pub_key) }.boxed().into());
        }
        let mut i_s = self.internal_sender.clone();
        Ok(async move {
            let (pub_key, sec_key) = crypto_sign_keypair(None).await?;
            let pub_key = pub_key.read().to_vec();
            let agent_hash = holo_hash::AgentHash::with_pre_hashed(pub_key).await;
            let sec_key = PrivateKey(sec_key);
            i_s.ghost_actor_internal()
                .finalize_new_keypair((agent_hash.clone(), sec_key))
                .await?;
            Ok(agent_hash)
        }
        .boxed()
        .into())
    }

    fn handle_list_sign_keys(
        &mut self,
    ) -> Result<KeystoreFuture<Vec<holo_hash::AgentHash>>, KeystoreError> {
        let keys = self.active_keypairs.keys().cloned().collect();
        Ok(async move { Ok(keys) }.boxed().into())
    }

    fn handle_sign(
        &mut self,
        input: SignInput,
    ) -> Result<KeystoreFuture<Signature>, KeystoreError> {
        let SignInput { key, data } = input;
        let mut data = crypto_insecure_buffer_from_bytes(data.bytes())?;
        let mut sec_key = match self.active_keypairs.get(&key) {
            Some(sec_key) => sec_key.0.clone(),
            None => return Err(format!("Signature Failure, Unknown Agent: {}", key).into()),
        };
        Ok(async move {
            let signature = crypto_sign(&mut data, &mut sec_key).await?;
            let signature = signature.read().to_vec();
            Ok(Signature(signature))
        }
        .boxed()
        .into())
    }

    fn handle_ghost_actor_internal(&mut self, msg: MockKeystoreInternal) {
        match msg {
            MockKeystoreInternal::FinalizeNewKeypair(msg) => {
                let ghost_actor::GhostChanItem {
                    input,
                    respond,
                    span,
                } = msg;
                let _g = span.enter();
                let (agent_hash, sec_key) = input;
                self.active_keypairs.insert(agent_hash, sec_key);
                if let Err(e) = respond(Ok(())) {
                    ghost_actor::dependencies::tracing::error!(error = ?e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_keypairs() -> Vec<MockKeypair> {
        vec![
            MockKeypair {
                pub_key: holo_hash::AgentHash::try_from(
                    "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4",
                )
                .unwrap(),
                sec_key: vec![
                    220, 218, 15, 212, 178, 51, 204, 96, 121, 97, 6, 205, 179, 84, 80, 159, 84,
                    163, 193, 46, 127, 15, 47, 91, 134, 106, 72, 72, 51, 76, 26, 16, 195, 236, 235,
                    182, 216, 152, 165, 215, 192, 97, 126, 31, 71, 165, 188, 12, 245, 29, 133, 230,
                    73, 251, 84, 44, 68, 14, 28, 76, 137, 166, 205, 54,
                ],
            },
            MockKeypair {
                pub_key: holo_hash::AgentHash::try_from(
                    "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK",
                )
                .unwrap(),
                sec_key: vec![
                    170, 205, 134, 46, 233, 225, 100, 162, 101, 124, 207, 157, 12, 131, 239, 244,
                    216, 190, 244, 161, 209, 56, 159, 135, 240, 134, 88, 28, 48, 75, 227, 244, 162,
                    97, 243, 122, 69, 52, 251, 30, 233, 235, 101, 166, 174, 235, 29, 196, 61, 176,
                    247, 7, 35, 117, 168, 194, 243, 206, 188, 240, 145, 146, 76, 74,
                ],
            },
        ]
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_mock_keystore_can_supply_fixture_keys() {
        let _ = crypto_init_sodium();
        tokio::task::spawn(async move {
            let (mut keystore, driver) = spawn_mock_keystore(fixture_keypairs()).await.unwrap();
            tokio::task::spawn(driver);

            let agent1 = keystore
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap();
            let agent2 = keystore
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap();
            let agent3 = keystore
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap();

            assert_eq!(
                "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4",
                &agent1.to_string(),
            );
            assert_eq!(
                "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK",
                &agent2.to_string(),
            );
            assert_ne!(&agent1.to_string(), &agent3.to_string(),);

            let mut sign_keys = keystore
                .list_sign_keys()
                .await
                .unwrap()
                .iter()
                .map(|agent| agent.to_string())
                .collect::<Vec<_>>();
            sign_keys.sort();

            let mut expected = vec![agent1.to_string(), agent2.to_string(), agent3.to_string()];
            expected.sort();

            assert_eq!(&format!("{:?}", expected), &format!("{:?}", sign_keys),);
        })
        .await
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_mock_keystore_can_sign_and_validate_data() {
        let _ = crypto_init_sodium();
        tokio::task::spawn(async move {
            let (mut keystore, driver) = spawn_mock_keystore(fixture_keypairs()).await.unwrap();
            tokio::task::spawn(driver);

            let agent_hash = keystore
                .generate_sign_keypair_from_pure_entropy()
                .await
                .unwrap();

            #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
            struct MyData(Vec<u8>);

            let my_data_1 = MyData(b"signature test data 1".to_vec());
            let my_data_2 = MyData(b"signature test data 2".to_vec());

            let signature = keystore
                .sign(SignInput::new(agent_hash.clone(), &my_data_1).unwrap())
                .await
                .unwrap();

            assert!(agent_hash
                .verify_signature(&signature, &my_data_1)
                .await
                .unwrap());
            assert!(!agent_hash
                .verify_signature(&signature, &my_data_2)
                .await
                .unwrap());
        })
        .await
        .unwrap();
    }
}
