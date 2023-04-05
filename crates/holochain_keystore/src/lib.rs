#![deny(missing_docs)]
#![allow(clippy::needless_doctest_main)]
//! A Keystore is a secure repository of private keys. MetaLairClient is a
//! reference to a Keystore. MetaLairClient allows async generation of keypairs,
//! and usage of those keypairs, reference by the public AgentPubKey.
//!
//! # Examples
//!
//! ```
//! use holo_hash::AgentPubKey;
//! use holochain_keystore::*;
//! use holochain_serialized_bytes::prelude::*;
//!
//! #[tokio::main(flavor = "multi_thread")]
//! async fn main() {
//!     tokio::task::spawn(async move {
//!         let keystore = test_keystore::spawn_test_keystore().await.unwrap();
//!         let agent_pubkey = AgentPubKey::new_random(&keystore).await.unwrap();
//!
//!         #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
//!         struct MyData(Vec<u8>);
//!
//!         let my_data_1 = MyData(b"signature test data 1".to_vec());
//!
//!         let signature = agent_pubkey.sign(&keystore, &my_data_1).await.unwrap();
//!
//!         assert!(agent_pubkey.verify_signature(&signature, &my_data_1).await);
//!     }).await.unwrap();
//! }
//! ```

use holochain_serialized_bytes::prelude::*;

mod error;
pub use error::*;

mod meta_lair_client;
pub use meta_lair_client::*;

mod agent_pubkey_ext;
pub use agent_pubkey_ext::*;

pub mod crude_mock_keystore;
pub mod lair_keystore;
pub mod test_keystore;

/// Construct a simple in-memory in-process keystore.
pub async fn spawn_mem_keystore() -> LairResult<MetaLairClient> {
    use kitsune_p2p_types::dependencies::lair_keystore_api;
    use lair_keystore_api::prelude::*;
    use std::sync::Arc;

    // in-memory secure random passphrase
    let passphrase = sodoken::BufWrite::new_mem_locked(32)?;
    sodoken::random::bytes_buf(passphrase.clone()).await?;

    // in-mem / in-proc config
    let config = Arc::new(
        PwHashLimits::Minimum
            .with_exec(|| {
                lair_keystore_api::config::LairServerConfigInner::new("/", passphrase.to_read())
            })
            .await?,
    );

    // the keystore
    let keystore = lair_keystore_api::in_proc_keystore::InProcKeystore::new(
        config,
        lair_keystore_api::mem_store::create_mem_store_factory(),
        passphrase.to_read(),
    )
    .await?;

    // return the client
    let client = keystore.new_client().await?;
    let (s, _) = tokio::sync::mpsc::unbounded_channel();
    Ok(MetaLairClient(Arc::new(parking_lot::Mutex::new(client)), s))
}
