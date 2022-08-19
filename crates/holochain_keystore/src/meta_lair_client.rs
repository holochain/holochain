use holochain_zome_types::Signature;
use kitsune_p2p_types::dependencies::{lair_keystore_api, url2};
use lair_keystore_api::prelude::*;
use parking_lot::Mutex;
use std::future::Future;
use std::sync::Arc;

pub use kitsune_p2p_types::dependencies::lair_keystore_api::LairResult;

const TIME_CHECK_FREQ: std::time::Duration = std::time::Duration::from_secs(5);
const CON_CHECK_STUB_TAG: &str = "HC_CON_CHK_STUB";
const RECON_INIT_MS: u64 = 100;
const RECON_MAX_MS: u64 = 5000;

type Esnd = tokio::sync::mpsc::UnboundedSender<()>;

/// Abstraction around runtime switching/upgrade of lair keystore / client.
#[derive(Clone)]
pub struct MetaLairClient(pub(crate) Arc<Mutex<LairClient>>, pub(crate) Esnd);

macro_rules! echk {
    ($esnd:ident, $code:expr) => {{
        match $code {
            Err(err) => {
                let _ = $esnd.send(());
                return Err(err);
            }
            Ok(r) => r,
        }
    }};
}

impl MetaLairClient {
    pub(crate) async fn new(
        connection_url: url2::Url2,
        passphrase: sodoken::BufRead,
    ) -> LairResult<Self> {
        use lair_keystore_api::ipc_keystore::*;
        let opts = IpcKeystoreClientOptions {
            connection_url: connection_url.clone().into(),
            passphrase: passphrase.clone(),
            exact_client_server_version_match: true,
        };

        let client = ipc_keystore_connect_options(opts).await?;
        let inner = Arc::new(Mutex::new(client));

        let (c_check_send, mut c_check_recv) = tokio::sync::mpsc::unbounded_channel();
        // initial check
        let _ = c_check_send.send(());

        // setup timeout for connection check
        {
            let c_check_send = c_check_send.clone();
            tokio::task::spawn(async move {
                loop {
                    tokio::time::sleep(TIME_CHECK_FREQ).await;
                    if c_check_send.send(()).is_err() {
                        break;
                    }
                }
            });
        }

        // setup the connection check logic
        {
            let inner = inner.clone();
            let stub_tag: Arc<str> = CON_CHECK_STUB_TAG.to_string().into();
            tokio::task::spawn(async move {
                use tokio::sync::mpsc::error::TryRecvError;
                'top_loop: while c_check_recv.recv().await.is_some() {
                    'drain_queue: loop {
                        match c_check_recv.try_recv() {
                            Ok(_) => (),
                            Err(TryRecvError::Empty) => break 'drain_queue,
                            Err(TryRecvError::Disconnected) => break 'top_loop,
                        }
                    }

                    let client = inner.lock().clone();

                    // optimistic check - most often the stub will be there
                    if client.get_entry(stub_tag.clone()).await.is_ok() {
                        continue;
                    }

                    // on the first run of a new install we need to create
                    let _ = client.new_seed(stub_tag.clone(), None, false).await;

                    // then we can exit early again
                    if client.get_entry(stub_tag.clone()).await.is_ok() {
                        continue;
                    }

                    // we couldn't fetch the stub, enter our reconnect loop
                    let mut backoff_ms = RECON_INIT_MS;
                    'reconnect: loop {
                        'drain_queue2: loop {
                            match c_check_recv.try_recv() {
                                Ok(_) => (),
                                Err(TryRecvError::Empty) => break 'drain_queue2,
                                Err(TryRecvError::Disconnected) => break 'top_loop,
                            }
                        }

                        backoff_ms *= 2;
                        if backoff_ms >= RECON_MAX_MS {
                            backoff_ms = RECON_MAX_MS;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                        let opts = IpcKeystoreClientOptions {
                            connection_url: connection_url.clone().into(),
                            passphrase: passphrase.clone(),
                            exact_client_server_version_match: true,
                        };

                        tracing::warn!("lair connection lost, attempting reconnect");

                        let client = match ipc_keystore_connect_options(opts).await {
                            Err(err) => {
                                tracing::error!(?err, "lair connect error");
                                continue 'reconnect;
                            }
                            Ok(client) => client,
                        };

                        *inner.lock() = client;

                        tracing::info!("lair reconnect success");

                        break 'reconnect;
                    }
                }
            });
        }

        Ok(MetaLairClient(inner, c_check_send))
    }

    pub(crate) fn cli(&self) -> (LairClient, Esnd) {
        (self.0.lock().clone(), self.1.clone())
    }

    /// Shutdown this keystore client
    pub fn shutdown(&self) -> impl Future<Output = LairResult<()>> + 'static + Send {
        let (client, _esnd) = self.cli();
        async move { client.shutdown().await }
    }

    /// Construct a new randomized signature keypair
    pub fn new_sign_keypair_random(
        &self,
    ) -> impl Future<Output = LairResult<holo_hash::AgentPubKey>> + 'static + Send {
        let (client, esnd) = self.cli();
        async move {
            let tag = nanoid::nanoid!();
            let info = echk!(esnd, client.new_seed(tag.into(), None, false).await);
            let pub_key = holo_hash::AgentPubKey::from_raw_32(info.ed25519_pub_key.0.to_vec());
            Ok(pub_key)
        }
    }

    /// Generate a new signature for given keypair / data
    pub fn sign(
        &self,
        pub_key: holo_hash::AgentPubKey,
        data: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<Signature>> + 'static + Send {
        let (client, esnd) = self.cli();
        async move {
            tokio::time::timeout(std::time::Duration::from_secs(30), async move {
                let mut pub_key_2 = [0; 32];
                pub_key_2.copy_from_slice(pub_key.get_raw_32());
                let sig = echk!(
                    esnd,
                    client.sign_by_pub_key(pub_key_2.into(), None, data).await
                );
                Ok(Signature(*sig.0))
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
        let (client, esnd) = self.cli();
        async move {
            // shared secrets are exportable
            // (it's hard to make them useful otherwise : )
            let exportable = true;
            let _info = echk!(esnd, client.new_seed(tag, None, exportable).await);
            Ok(())
        }
    }

    /// Export a shared secret identified by `tag` using box encryption.
    pub fn shared_secret_export(
        &self,
        tag: Arc<str>,
        sender_pub_key: X25519PubKey,
        recipient_pub_key: X25519PubKey,
    ) -> impl Future<Output = LairResult<([u8; 24], Arc<[u8]>)>> + 'static + Send {
        let (client, esnd) = self.cli();
        async move {
            Ok(echk!(
                esnd,
                client
                    .export_seed_by_tag(tag, sender_pub_key, recipient_pub_key, None)
                    .await
            ))
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
        let (client, esnd) = self.cli();
        async move {
            // shared secrets are exportable
            // (it's hard to make them useful otherwise : )
            let exportable = true;
            let _info = echk!(
                esnd,
                client
                    .import_seed(
                        sender_pub_key,
                        recipient_pub_key,
                        None,
                        nonce,
                        cipher,
                        tag,
                        exportable,
                    )
                    .await
            );
            Ok(())
        }
    }

    /// Encrypt using a shared secret / xsalsa20poly1305 secretbox.
    pub fn shared_secret_encrypt(
        &self,
        tag: Arc<str>,
        data: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<([u8; 24], Arc<[u8]>)>> + 'static + Send {
        let (client, esnd) = self.cli();
        async move {
            Ok(echk!(
                esnd,
                client.secretbox_xsalsa_by_tag(tag, None, data).await
            ))
        }
    }

    /// Decrypt using a shared secret / xsalsa20poly1305 secretbox.
    pub fn shared_secret_decrypt(
        &self,
        tag: Arc<str>,
        nonce: [u8; 24],
        cipher: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<Arc<[u8]>>> + 'static + Send {
        let (client, esnd) = self.cli();
        async move {
            Ok(echk!(
                esnd,
                client
                    .secretbox_xsalsa_open_by_tag(tag, None, nonce, cipher)
                    .await
            ))
        }
    }

    /// Construct a new randomized encryption keypair
    pub fn new_x25519_keypair_random(
        &self,
    ) -> impl Future<Output = LairResult<X25519PubKey>> + 'static + Send {
        let (client, esnd) = self.cli();
        async move {
            let tag = nanoid::nanoid!();
            let info = echk!(esnd, client.new_seed(tag.into(), None, false).await);
            let pub_key = info.x25519_pub_key;
            Ok(pub_key)
        }
    }

    /// Encrypt an authenticated "box"ed message to a specific recipient.
    pub fn crypto_box_xsalsa(
        &self,
        sender_pub_key: X25519PubKey,
        recipient_pub_key: X25519PubKey,
        data: Arc<[u8]>,
    ) -> impl Future<Output = LairResult<([u8; 24], Arc<[u8]>)>> + 'static + Send {
        let (client, esnd) = self.cli();
        async move {
            Ok(echk!(
                esnd,
                client
                    .crypto_box_xsalsa_by_pub_key(sender_pub_key, recipient_pub_key, None, data)
                    .await
            ))
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
        let (client, esnd) = self.cli();
        async move {
            Ok(echk!(
                esnd,
                client
                    .crypto_box_xsalsa_open_by_pub_key(
                        sender_pub_key,
                        recipient_pub_key,
                        None,
                        nonce,
                        data,
                    )
                    .await
            ))
        }
    }

    /// Get a tls cert from lair for use in conductor
    pub fn get_or_create_tls_cert_by_tag(
        &self,
        tag: Arc<str>,
    ) -> impl Future<Output = LairResult<(CertDigest, Arc<[u8]>, sodoken::BufRead)>> + 'static + Send
    {
        let (client, esnd) = self.cli();
        async move {
            // don't echk! this top one, it may be a valid error
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
                Err(_) => {
                    let esnd = esnd.clone();
                    echk!(esnd, client.new_wka_tls_cert(tag.clone()).await)
                }
            };
            let pk = echk!(esnd, client.get_wka_tls_cert_priv_key(tag).await);

            Ok((info.digest, info.cert.to_vec().into(), pk))
        }
    }
}
