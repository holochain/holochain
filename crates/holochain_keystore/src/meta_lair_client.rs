use crate::*;
use holochain_zome_types::Signature;
use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::prelude::*;
use lair_keystore_api_0_0::actor::Cert as LegacyCert;
use lair_keystore_api_0_0::actor::CertDigest as LegacyCertDigest;
use lair_keystore_api_0_0::actor::CertPrivKey as LegacyCertPrivKey;
use lair_keystore_api_0_0::actor::LairClientApiSender;
use std::future::Future;
use std::sync::Arc;

pub use kitsune_p2p_types::dependencies::lair_keystore_api::LairResult;

/// Abstraction around runtime switching/upgrade of lair keystore / client.
/// Can delete this when we finally delete deprecated legacy lair option.
#[derive(Clone)]
pub enum MetaLairClient {
    /// oldschool deprecated lair keystore client
    Legacy(KeystoreSender),

    /// new lair keystore api client
    NewLair(LairClient),
}

impl MetaLairClient {
    /// Shutdown this keystore client
    pub fn shutdown(&self) -> impl Future<Output = LairResult<()>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(client) => {
                    use ghost_actor::GhostControlSender;
                    client
                        .ghost_actor_shutdown_immediate()
                        .await
                        .map_err(one_err::OneErr::new)
                }
                Self::NewLair(client) => client.shutdown().await,
            }
        }
    }

    /// Construct a new randomized signature keypair
    pub fn new_sign_keypair_random(
        &self,
    ) -> impl Future<Output = LairResult<holo_hash::AgentPubKey>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(client) => {
                    let (_, pk) = client
                        .sign_ed25519_new_from_entropy()
                        .await
                        .map_err(one_err::OneErr::new)?;
                    Ok(holo_hash::AgentPubKey::from_raw_32(pk.to_vec()))
                }
                Self::NewLair(client) => {
                    let tag = nanoid::nanoid!();
                    let info = client.new_seed(tag.into(), None, false).await?;
                    let pub_key =
                        holo_hash::AgentPubKey::from_raw_32(info.ed25519_pub_key.0.to_vec());
                    Ok(pub_key)
                }
            }
        }
    }

    /// Generate a new signature for given keypair / data
    pub fn sign(
        &self,
        pub_key: holo_hash::AgentPubKey,
        data: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<Signature>> + 'static + Send {
        let this = self.clone();
        async move {
            tokio::time::timeout(std::time::Duration::from_secs(30), async move {
                match this {
                    Self::Legacy(client) => {
                        let pk = pub_key.get_raw_32();
                        let sig = client
                            .sign_ed25519_sign_by_pub_key(pk.to_vec().into(), data.to_vec().into())
                            .await
                            .map_err(one_err::OneErr::new)?;
                        let sig = Signature::try_from(sig.to_vec().as_ref())
                            .map_err(one_err::OneErr::new)?;
                        Ok(sig)
                    }
                    Self::NewLair(client) => {
                        let mut pub_key_2 = [0; 32];
                        pub_key_2.copy_from_slice(pub_key.get_raw_32());
                        let sig = client.sign_by_pub_key(pub_key_2.into(), None, data).await?;
                        Ok(Signature(*sig.0))
                    }
                }
            })
            .await
            .map_err(one_err::OneErr::new)?
        }
    }

    /// Construct a new randomized shared secret, associated with given tag
    pub fn new_shared_secret(
        &self,
        tag: Arc<str>,
    ) -> impl Future<Output = LairResult<()>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(_) => Err("LegacyLairDoesNotSupportSharedSecrets".into()),
                Self::NewLair(client) => {
                    // shared secrets are exportable
                    // (it's hard to make them useful otherwise : )
                    let exportable = true;
                    let _info = client.new_seed(tag, None, exportable).await?;
                    Ok(())
                }
            }
        }
    }

    /// Construct a new randomized encryption keypair
    pub fn new_x25519_keypair_random(
        &self,
    ) -> impl Future<Output = LairResult<X25519PubKey>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(client) => {
                    let (_, pk) = client
                        .x25519_new_from_entropy()
                        .await
                        .map_err(one_err::OneErr::new)?;
                    Ok(pk.to_bytes().into())
                }
                Self::NewLair(client) => {
                    let tag = nanoid::nanoid!();
                    let info = client.new_seed(tag.into(), None, false).await?;
                    let pub_key = info.x25519_pub_key;
                    Ok(pub_key)
                }
            }
        }
    }

    /// Encrypt an authenticated "box"ed message to a specific recipient.
    pub fn crypto_box_xsalsa(
        &self,
        sender_pub_key: X25519PubKey,
        recipient_pub_key: X25519PubKey,
        data: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<([u8; 24], Arc<[u8]>)>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(client) => {
                    let res = client
                        .crypto_box_by_pub_key(
                            (*sender_pub_key).into(),
                            (*recipient_pub_key).into(),
                            lair_keystore_api_0_0::internal::crypto_box::CryptoBoxData {
                                data: data.to_vec().into(),
                            }
                            .into(),
                        )
                        .await
                        .map_err(one_err::OneErr::new)?;
                    Ok((*res.nonce.as_ref(), res.encrypted_data.to_vec().into()))
                }
                Self::NewLair(client) => {
                    client
                        .crypto_box_xsalsa_by_pub_key(sender_pub_key, recipient_pub_key, None, data)
                        .await
                }
            }
        }
    }

    /// Decrypt an authenticated "box"ed message from a specific sender.
    pub fn crypto_box_xsalsa_open(
        &self,
        sender_pub_key: X25519PubKey,
        recipient_pub_key: X25519PubKey,
        nonce: [u8; 24],
        data: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<Arc<[u8]>>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(client) => {
                    let res = client
                        .crypto_box_open_by_pub_key(
                            (*sender_pub_key).into(),
                            (*recipient_pub_key).into(),
                            lair_keystore_api_0_0::internal::crypto_box::CryptoBoxEncryptedData {
                                nonce: nonce.into(),
                                encrypted_data: data.to_vec().into(),
                            }
                            .into(),
                        )
                        .await
                        .map_err(one_err::OneErr::new)?;
                    let res = res.ok_or_else(|| {
                        one_err::OneErr::new(
                            "None returned from crypto_box_open--why is that an Option??",
                        )
                    })?;
                    Ok(res.data.to_vec().into())
                }
                Self::NewLair(client) => {
                    client
                        .crypto_box_xsalsa_open_by_pub_key(
                            sender_pub_key,
                            recipient_pub_key,
                            None,
                            nonce,
                            data,
                        )
                        .await
                }
            }
        }
    }

    /// Get a single tls cert from lair for use in conductor
    /// NOTE: once we delete the deprecated legacy lair api
    /// we can support multiple conductors using the same lair
    /// by tagging the tls certs / remembering the tag.
    pub fn get_or_create_first_tls_cert(
        &self,
    ) -> impl Future<Output = LairResult<(LegacyCertDigest, LegacyCert, LegacyCertPrivKey)>>
           + 'static
           + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Legacy(client) => client
                    .get_or_create_first_tls_cert()
                    .await
                    .map_err(one_err::OneErr::new),
                Self::NewLair(client) => {
                    const ONE_CERT: &str = "SingleHcTlsWkaCert";
                    let info = match client.get_entry(ONE_CERT.into()).await {
                        Ok(info) => match info {
                            LairEntryInfo::WkaTlsCert { cert_info, .. } => cert_info,
                            oth => {
                                return Err(format!(
                                    "invalid entry type, expecting wka tls cert: {:?}",
                                    oth
                                )
                                .into())
                            }
                        },
                        Err(_) => client.new_wka_tls_cert(ONE_CERT.into()).await?,
                    };
                    let pk = client.get_wka_tls_cert_priv_key(ONE_CERT.into()).await?;

                    let digest: LegacyCertDigest = info.digest.to_vec().into();
                    let cert: LegacyCert = info.cert.to_vec().into();

                    // is it worth trying to keep this safe longer?
                    // i doubt our tls lib safes this in-memory...
                    let pk: LegacyCertPrivKey = pk.read_lock().to_vec().into();

                    Ok((digest, cert, pk))
                }
            }
        }
    }
}
