use holochain_zome_types::Signature;
use kitsune_p2p_types::dependencies::lair_keystore_api;
use lair_keystore_api::prelude::*;
use std::future::Future;
use std::sync::Arc;

pub use kitsune_p2p_types::dependencies::lair_keystore_api::LairResult;

/// Abstraction around runtime switching/upgrade of lair keystore / client.
#[derive(Clone)]
pub enum MetaLairClient {
    /// lair keystore api client
    Lair(LairClient),
}

impl MetaLairClient {
    /// Shutdown this keystore client
    pub fn shutdown(&self) -> impl Future<Output = LairResult<()>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Lair(client) => client.shutdown().await,
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
                Self::Lair(client) => {
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
                    Self::Lair(client) => {
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
                Self::Lair(client) => {
                    // shared secrets are exportable
                    // (it's hard to make them useful otherwise : )
                    let exportable = true;
                    let _info = client.new_seed(tag, None, exportable).await?;
                    Ok(())
                }
            }
        }
    }

    /// Export a shared secret identified by `tag` using box encryption.
    pub fn shared_secret_export(
        &self,
        tag: Arc<str>,
        sender_pub_key: X25519PubKey,
        recipient_pub_key: X25519PubKey,
    ) -> impl Future<Output = LairResult<([u8; 24], Arc<[u8]>)>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Lair(client) => Ok(client
                    .export_seed_by_tag(tag, sender_pub_key, recipient_pub_key, None)
                    .await?),
            }
        }
    }

    /// Import a shared secret to be indentified by `tag` using box decryption.
    pub fn shared_secret_import(
        &self,
        sender_pub_key: X25519PubKey,
        recipient_pub_key: X25519PubKey,
        nonce: [u8; 24],
        cipher: Arc<[u8]>,
        tag: Arc<str>,
    ) -> impl Future<Output = LairResult<()>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Lair(client) => {
                    // shared secrets are exportable
                    // (it's hard to make them useful otherwise : )
                    let exportable = true;
                    let _info = client
                        .import_seed(
                            sender_pub_key,
                            recipient_pub_key,
                            None,
                            nonce,
                            cipher,
                            tag,
                            exportable,
                        )
                        .await?;
                    Ok(())
                }
            }
        }
    }

    /// Encrypt using a shared secret / xsalsa20poly1305 secretbox.
    pub fn shared_secret_encrypt(
        &self,
        tag: Arc<str>,
        data: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<([u8; 24], Arc<[u8]>)>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Lair(client) => client.secretbox_xsalsa_by_tag(tag, None, data).await,
            }
        }
    }

    /// Decrypt using a shared secret / xsalsa20poly1305 secretbox.
    pub fn shared_secret_decrypt(
        &self,
        tag: Arc<str>,
        nonce: [u8; 24],
        cipher: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<Arc<[u8]>>> + 'static + Send {
        let this = self.clone();
        async move {
            match this {
                Self::Lair(client) => {
                    client
                        .secretbox_xsalsa_open_by_tag(tag, None, nonce, cipher)
                        .await
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
                Self::Lair(client) => {
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
                Self::Lair(client) => {
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
                Self::Lair(client) => {
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

    /// Get a tls cert from lair for use in conductor
    pub fn get_or_create_tls_cert_by_tag(
        &self,
        tag: Arc<str>,
    ) -> impl Future<Output = LairResult<(CertDigest, Arc<[u8]>, sodoken::BufRead)>> + 'static + Send
    {
        let this = self.clone();
        async move {
            match this {
                Self::Lair(client) => {
                    let info = match client.get_entry(tag.clone()).await {
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
                        Err(_) => client.new_wka_tls_cert(tag.clone()).await?,
                    };
                    let pk = client.get_wka_tls_cert_priv_key(tag).await?;

                    Ok((info.digest, info.cert.to_vec().into(), pk))
                }
            }
        }
    }
}
