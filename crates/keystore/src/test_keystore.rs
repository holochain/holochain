//! DANGER! This is a mock keystore for testing, DO NOT USE THIS IN PRODUCTION!

use crate::*;
use ghost_actor::dependencies::futures::future::FutureExt;
use holochain_crypto::*;
use lair_keystore_api::actor::*;
use lair_keystore_api::*;
use std::collections::HashMap;
use std::sync::Arc;
impl ghost_actor::GhostHandler<LairClientApi> for TestKeystore {}

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
    let _sender = builder
        .channel_factory()
        .create_channel::<KeystoreApi>()
        .await?;
    let sender = builder
        .channel_factory()
        .create_channel::<LairClientApi>()
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
            idx: u32,
            pub_key: holo_hash::AgentPubKey,
            priv_key: PrivateKey,
        ) -> ();
    }
}

/// Internal mock keystore struct.
struct TestKeystore {
    internal_sender: ghost_actor::GhostSender<TestKeystoreInternal>,
    fixture_keypairs: Vec<MockKeypair>,
    active_keypairs: HashMap<holo_hash::AgentPubKey, (u32, PrivateKey)>,
    next_idx: u32,
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
            next_idx: 0,
        }
    }

    fn next_idx(&mut self) -> u32 {
        let out = self.next_idx;
        self.next_idx += 1;
        out
    }
}

impl ghost_actor::GhostControlHandler for TestKeystore {}

impl ghost_actor::GhostHandler<TestKeystoreInternal> for TestKeystore {}

impl TestKeystoreInternalHandler for TestKeystore {
    fn handle_finalize_new_keypair(
        &mut self,
        idx: u32,
        pub_key: holo_hash::AgentPubKey,
        priv_key: PrivateKey,
    ) -> TestKeystoreInternalHandlerResult<()> {
        self.active_keypairs.insert(pub_key, (idx, priv_key));
        Ok(async move { Ok(()) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<KeystoreApi> for TestKeystore {}

impl KeystoreApiHandler for TestKeystore {
    fn handle_generate_sign_keypair_from_pure_entropy(
        &mut self,
    ) -> KeystoreApiHandlerResult<holo_hash::AgentPubKey> {
        let fut = self.handle_sign_ed25519_new_from_entropy()?;
        Ok(async move {
            let (_, pk) = fut.await?;
            Ok(holo_hash::AgentPubKey::with_pre_hashed(pk.to_vec()))
        }
        .boxed()
        .into())
    }

    fn handle_sign(&mut self, input: SignInput) -> KeystoreApiHandlerResult<Signature> {
        let fut = self.handle_sign_ed25519_sign_by_pub_key(
            input.key.as_ref().to_vec().into(),
            <Vec<u8>>::from(UnsafeBytes::from(input.data)).into(),
        )?;
        Ok(async move {
            let res = fut.await?;
            Ok(Signature(res.to_vec()))
        }
        .boxed()
        .into())
    }
}

impl LairClientApiHandler for TestKeystore {
    fn handle_lair_get_server_info(&mut self) -> LairClientApiHandlerResult<LairServerInfo> {
        unimplemented!()
    }
    fn handle_lair_get_last_entry_index(&mut self) -> LairClientApiHandlerResult<KeystoreIndex> {
        unimplemented!()
    }
    fn handle_lair_get_entry_type(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<LairEntryType> {
        unimplemented!()
    }
    fn handle_tls_cert_new_self_signed_from_entropy(
        &mut self,
        _options: TlsCertOptions,
    ) -> LairClientApiHandlerResult<(KeystoreIndex, CertSni, CertDigest)> {
        unimplemented!()
    }
    fn handle_tls_cert_get(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<(CertSni, CertDigest)> {
        unimplemented!()
    }
    fn handle_tls_cert_get_cert_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<Cert> {
        unimplemented!()
    }
    fn handle_tls_cert_get_cert_by_digest(
        &mut self,
        _cert_digest: CertDigest,
    ) -> LairClientApiHandlerResult<Cert> {
        unimplemented!()
    }
    fn handle_tls_cert_get_cert_by_sni(
        &mut self,
        _cert_sni: CertSni,
    ) -> LairClientApiHandlerResult<Cert> {
        unimplemented!()
    }
    fn handle_tls_cert_get_priv_key_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<CertPrivKey> {
        unimplemented!()
    }
    fn handle_tls_cert_get_priv_key_by_digest(
        &mut self,
        _cert_digest: CertDigest,
    ) -> LairClientApiHandlerResult<CertPrivKey> {
        unimplemented!()
    }
    fn handle_tls_cert_get_priv_key_by_sni(
        &mut self,
        _cert_sni: CertSni,
    ) -> LairClientApiHandlerResult<CertPrivKey> {
        unimplemented!()
    }
    fn handle_sign_ed25519_new_from_entropy(
        &mut self,
    ) -> LairClientApiHandlerResult<(KeystoreIndex, SignEd25519PubKey)> {
        if !self.fixture_keypairs.is_empty() {
            let MockKeypair { pub_key, sec_key } = self.fixture_keypairs.remove(0);
            // we're loading this out of insecure memory - but this is just a mock
            let sec_key = PrivateKey(
                danger_crypto_secure_buffer_from_bytes(&sec_key).map_err(LairError::other)?,
            );
            let idx = self.next_idx();
            self.active_keypairs.insert(pub_key.clone(), (idx, sec_key));
            return Ok(
                async move { Ok((0.into(), pub_key.as_ref().to_vec().into())) }
                    .boxed()
                    .into(),
            );
        }
        let i_s = self.internal_sender.clone();
        let idx = self.next_idx();
        Ok(async move {
            let (pub_key, sec_key) = crypto_sign_keypair(None).await.map_err(LairError::other)?;
            let pub_key = pub_key.read().to_vec();
            let agent_pubkey = holo_hash::AgentPubKey::with_pre_hashed(pub_key);
            let sec_key = PrivateKey(sec_key);
            i_s.finalize_new_keypair(idx, agent_pubkey.clone(), sec_key)
                .await?;
            Ok((idx.into(), agent_pubkey.as_ref().to_vec().into()))
        }
        .boxed()
        .into())
    }
    fn handle_sign_ed25519_get(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<SignEd25519PubKey> {
        unimplemented!()
    }
    fn handle_sign_ed25519_sign_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
        _message: Arc<Vec<u8>>,
    ) -> LairClientApiHandlerResult<SignEd25519Signature> {
        unimplemented!()
    }
    fn handle_sign_ed25519_sign_by_pub_key(
        &mut self,
        pub_key: SignEd25519PubKey,
        message: Arc<Vec<u8>>,
    ) -> LairClientApiHandlerResult<SignEd25519Signature> {
        let pub_key = holo_hash::AgentPubKey::with_pre_hashed(pub_key.to_vec());
        let mut data = crypto_insecure_buffer_from_bytes(&message).map_err(LairError::other)?;
        let mut sec_key = match self.active_keypairs.get(&pub_key) {
            Some((_, sec_key)) => sec_key.0.clone(),
            None => return Err(format!("Signature Failure, Unknown Agent: {}", pub_key).into()),
        };
        Ok(async move {
            let signature = crypto_sign(&mut data, &mut sec_key)
                .await
                .map_err(LairError::other)?;
            let signature = signature.read().to_vec();
            Ok(signature.into())
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
            assert_ne!(&agent1.to_string(), &agent3.to_string());
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
