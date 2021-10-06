use crate::*;
use holochain_zome_types::Signature;
use kitsune_p2p_types::dependencies::new_lair_api;
use legacy_lair_api::actor::Cert as LegacyCert;
use legacy_lair_api::actor::CertDigest as LegacyCertDigest;
use legacy_lair_api::actor::CertPrivKey as LegacyCertPrivKey;
use legacy_lair_api::actor::LairClientApiSender;
use new_lair_api::prelude::*;
use std::future::Future;
use std::sync::Arc;

pub use kitsune_p2p_types::dependencies::new_lair_api::LairResult;

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
    /// TODO this is just a temp helper to ease implementing the switch
    pub fn unwrap_legacy(&self) -> &KeystoreSender {
        if let MetaLairClient::Legacy(client) = self {
            client
        } else {
            todo!()
        }
    }

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
                    let info = client.new_seed(tag.into(), None).await?;
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
