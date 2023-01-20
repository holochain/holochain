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
