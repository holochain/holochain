//! DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!

use crate::*;
use ghost_actor::dependencies::futures::future::FutureExt;
use holo_hash::HoloHashBaseExt;
use holochain_crypto::*;
use std::collections::HashMap;

/// DANGER! These Mock Keypairs should NEVER be used in production
/// The private keys have not been handled securely!
pub struct MockKeypair {
    /// The agent public key.
    pub pub_key: holo_hash::AgentPubKey,

    /// The private secret key DANGER - this is not handled securely!!
    pub sec_key: Vec<u8>,
}

/// Construct a new TestKeystore.
/// DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!
pub async fn spawn_test_keystore(
    fixture_keypairs: Vec<MockKeypair>,
) -> KeystoreApiResult<KeystoreSender> {
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();
    let internal_sender = builder
        .channel_factory()
        .create_channel::<TestKeystoreInternal>()
        .await?;
    let sender = builder
        .channel_factory()
        .create_channel::<KeystoreApi>()
        .await?;
    tokio::task::spawn(builder.spawn(TestKeystore::new(internal_sender, fixture_keypairs)));
    Ok(sender)
}

/// Internal Private Key newtype.
#[derive(Debug)]
struct PrivateKey(pub holochain_crypto::DynCryptoBytes);

ghost_actor::ghost_chan! {
    /// Internal Channel
    chan TestKeystoreInternal<KeystoreError> {
        /// we have generated a keypair, now track it
        fn finalize_new_keypair(
            pub_key: holo_hash::AgentPubKey,
            priv_key: PrivateKey,
        ) -> ();
    }
}

/// Internal mock keystore struct.
struct TestKeystore {
    internal_sender: ghost_actor::GhostSender<TestKeystoreInternal>,
    fixture_keypairs: Vec<MockKeypair>,
    active_keypairs: HashMap<holo_hash::AgentPubKey, PrivateKey>,
}

impl TestKeystore {
    fn new(
        internal_sender: ghost_actor::GhostSender<TestKeystoreInternal>,
        fixture_keypairs: Vec<MockKeypair>,
    ) -> Self {
        Self {
            internal_sender,
            fixture_keypairs,
            active_keypairs: HashMap::new(),
        }
    }
}

impl ghost_actor::GhostControlHandler for TestKeystore {}

impl ghost_actor::GhostHandler<TestKeystoreInternal> for TestKeystore {}

impl TestKeystoreInternalHandler for TestKeystore {
    fn handle_finalize_new_keypair(
        &mut self,
        pub_key: holo_hash::AgentPubKey,
        priv_key: PrivateKey,
    ) -> TestKeystoreInternalHandlerResult<()> {
        self.active_keypairs.insert(pub_key, priv_key);
        Ok(async move { Ok(()) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<KeystoreApi> for TestKeystore {}

impl KeystoreApiHandler for TestKeystore {
    fn handle_generate_sign_keypair_from_pure_entropy(
        &mut self,
    ) -> KeystoreApiHandlerResult<holo_hash::AgentPubKey> {
        if !self.fixture_keypairs.is_empty() {
            let MockKeypair { pub_key, sec_key } = self.fixture_keypairs.remove(0);
            // we're loading this out of insecure memory - but this is just a mock
            let sec_key = PrivateKey(danger_crypto_secure_buffer_from_bytes(&sec_key)?);
            self.active_keypairs.insert(pub_key.clone(), sec_key);
            return Ok(async move { Ok(pub_key) }.boxed().into());
        }
        let i_s = self.internal_sender.clone();
        Ok(async move {
            let (pub_key, sec_key) = crypto_sign_keypair(None).await?;
            let pub_key = pub_key.read().to_vec();
            let agent_pubkey = holo_hash::AgentPubKey::with_pre_hashed(pub_key);
            let sec_key = PrivateKey(sec_key);
            i_s.finalize_new_keypair(agent_pubkey.clone(), sec_key)
                .await?;
            Ok(agent_pubkey)
        }
        .boxed()
        .into())
    }

    fn handle_list_sign_keys(&mut self) -> KeystoreApiHandlerResult<Vec<holo_hash::AgentPubKey>> {
        let keys = self.active_keypairs.keys().cloned().collect();
        Ok(async move { Ok(keys) }.boxed().into())
    }

    fn handle_sign(&mut self, input: SignInput) -> KeystoreApiHandlerResult<Signature> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_keypairs() -> Vec<MockKeypair> {
        vec![
            MockKeypair {
                pub_key: holo_hash::AgentPubKey::try_from(
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
                pub_key: holo_hash::AgentPubKey::try_from(
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
    async fn test_test_keystore_can_supply_fixture_keys() {
        let _ = crypto_init_sodium();
        use holo_hash::AgentPubKey;
        tokio::task::spawn(async move {
            let keystore = spawn_test_keystore(fixture_keypairs()).await.unwrap();

            let agent1 = AgentPubKey::new_from_pure_entropy(&keystore).await.unwrap();
            let agent2 = AgentPubKey::new_from_pure_entropy(&keystore).await.unwrap();
            let agent3 = AgentPubKey::new_from_pure_entropy(&keystore).await.unwrap();

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
    async fn test_test_keystore_can_sign_and_validate_data() {
        let _ = crypto_init_sodium();
        use holo_hash::AgentPubKey;
        tokio::task::spawn(async move {
            let keystore = spawn_test_keystore(fixture_keypairs()).await.unwrap();

            let agent_pubkey = AgentPubKey::new_from_pure_entropy(&keystore).await.unwrap();

            #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
            struct MyData(Vec<u8>);

            let my_data_1 = MyData(b"signature test data 1".to_vec());
            let my_data_2 = MyData(b"signature test data 2".to_vec());

            let signature = agent_pubkey.sign(&keystore, &my_data_1).await.unwrap();

            assert!(agent_pubkey
                .verify_signature(&signature, &my_data_1)
                .await
                .unwrap());
            assert!(!agent_pubkey
                .verify_signature(&signature, &my_data_2)
                .await
                .unwrap());
        })
        .await
        .unwrap();
    }
}
