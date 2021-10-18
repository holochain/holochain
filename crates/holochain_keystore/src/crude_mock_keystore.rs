//! Defines a crude mock Keystore which always returns the same Error for every
//! call. This is about as close as we can get to a true mock which would allow
//! tweaking individual handlers, hence why this is a "crude" mock.

use std::sync::Arc;

use crate::lair_keystore_api_0_0::{actor::*, internal::*, *};

use crate::MetaLairClient;

/// Spawn a test keystore which always returns the same LairError for every call.
pub async fn spawn_crude_mock_keystore<F>(err_fn: F) -> LairResult<MetaLairClient>
where
    F: Fn() -> LairError + Send + 'static,
{
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let sender = builder
        .channel_factory()
        .create_channel::<LairClientApi>()
        .await?;

    tokio::task::spawn(builder.spawn(CrudeMockKeystore(Box::new(err_fn))));

    Ok(MetaLairClient::Legacy(sender))
}

/// A keystore which always returns the same LairError for every call.
struct CrudeMockKeystore(Box<dyn Fn() -> LairError + Send + 'static>);

impl ghost_actor::GhostControlHandler for CrudeMockKeystore {}
impl ghost_actor::GhostHandler<LairClientApi> for CrudeMockKeystore {}

impl LairClientApiHandler for CrudeMockKeystore {
    fn handle_lair_get_server_info(&mut self) -> LairClientApiHandlerResult<LairServerInfo> {
        Err(self.0())
    }

    fn handle_lair_get_last_entry_index(&mut self) -> LairClientApiHandlerResult<KeystoreIndex> {
        Err(self.0())
    }

    fn handle_lair_get_entry_type(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<LairEntryType> {
        Err(self.0())
    }

    fn handle_tls_cert_new_self_signed_from_entropy(
        &mut self,
        _options: TlsCertOptions,
    ) -> LairClientApiHandlerResult<(KeystoreIndex, CertSni, CertDigest)> {
        Err(self.0())
    }

    fn handle_tls_cert_get(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<(CertSni, CertDigest)> {
        Err(self.0())
    }

    fn handle_tls_cert_get_cert_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<Cert> {
        Err(self.0())
    }

    fn handle_tls_cert_get_cert_by_digest(
        &mut self,
        _cert_digest: CertDigest,
    ) -> LairClientApiHandlerResult<Cert> {
        Err(self.0())
    }

    fn handle_tls_cert_get_cert_by_sni(
        &mut self,
        _cert_sni: CertSni,
    ) -> LairClientApiHandlerResult<Cert> {
        Err(self.0())
    }

    fn handle_tls_cert_get_priv_key_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<CertPrivKey> {
        Err(self.0())
    }

    fn handle_tls_cert_get_priv_key_by_digest(
        &mut self,
        _cert_digest: CertDigest,
    ) -> LairClientApiHandlerResult<CertPrivKey> {
        Err(self.0())
    }

    fn handle_tls_cert_get_priv_key_by_sni(
        &mut self,
        _cert_sni: CertSni,
    ) -> LairClientApiHandlerResult<CertPrivKey> {
        Err(self.0())
    }

    fn handle_sign_ed25519_new_from_entropy(
        &mut self,
    ) -> LairClientApiHandlerResult<(KeystoreIndex, sign_ed25519::SignEd25519PubKey)> {
        Err(self.0())
    }

    fn handle_sign_ed25519_get(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<sign_ed25519::SignEd25519PubKey> {
        Err(self.0())
    }

    fn handle_sign_ed25519_sign_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
        _message: Arc<Vec<u8>>,
    ) -> LairClientApiHandlerResult<sign_ed25519::SignEd25519Signature> {
        Err(self.0())
    }

    fn handle_sign_ed25519_sign_by_pub_key(
        &mut self,
        _pub_key: sign_ed25519::SignEd25519PubKey,
        _message: Arc<Vec<u8>>,
    ) -> LairClientApiHandlerResult<sign_ed25519::SignEd25519Signature> {
        Err(self.0())
    }

    fn handle_x25519_new_from_entropy(
        &mut self,
    ) -> LairClientApiHandlerResult<(KeystoreIndex, x25519::X25519PubKey)> {
        Err(self.0())
    }

    fn handle_x25519_get(
        &mut self,
        _keystore_index: KeystoreIndex,
    ) -> LairClientApiHandlerResult<x25519::X25519PubKey> {
        Err(self.0())
    }

    fn handle_crypto_box_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
        _recipient: x25519::X25519PubKey,
        _data: Arc<crypto_box::CryptoBoxData>,
    ) -> LairClientApiHandlerResult<crypto_box::CryptoBoxEncryptedData> {
        Err(self.0())
    }

    fn handle_crypto_box_by_pub_key(
        &mut self,
        _pub_key: x25519::X25519PubKey,
        _recipient: x25519::X25519PubKey,
        _data: Arc<crypto_box::CryptoBoxData>,
    ) -> LairClientApiHandlerResult<crypto_box::CryptoBoxEncryptedData> {
        Err(self.0())
    }

    fn handle_crypto_box_open_by_index(
        &mut self,
        _keystore_index: KeystoreIndex,
        _sender: x25519::X25519PubKey,
        _encrypted_data: Arc<crypto_box::CryptoBoxEncryptedData>,
    ) -> LairClientApiHandlerResult<Option<crypto_box::CryptoBoxData>> {
        Err(self.0())
    }

    fn handle_crypto_box_open_by_pub_key(
        &mut self,
        _pub_key: x25519::X25519PubKey,
        _sender: x25519::X25519PubKey,
        _encrypted_data: Arc<crypto_box::CryptoBoxEncryptedData>,
    ) -> LairClientApiHandlerResult<Option<crypto_box::CryptoBoxData>> {
        Err(self.0())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_pubkey_ext::AgentPubKeyExt;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_crude_mock_keystore() {
        tokio::task::spawn(async move {
            let keystore = spawn_crude_mock_keystore(|| LairError::other("err"))
                .await
                .unwrap();

            assert_eq!(
                holo_hash::AgentPubKey::new_random(&keystore).await,
                Err(one_err::OneErr::new("err"))
            );
            // let agent = holo_hash::AgentPubKey::new_from_pure_entropy(&keystore)
            //     .await
            //     .unwrap();

            // #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
            // struct MyData(Vec<u8>);

            // let data = MyData(b"signature test data 1".to_vec());

            // assert_eq!(
            //     agent.sign(&keystore, &data).await,
            //     Err(KeystoreError::LairError(LairError::other("err")))
            // );
        })
        .await
        .unwrap();
    }
}
