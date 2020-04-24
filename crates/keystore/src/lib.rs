#![deny(missing_docs)]
#![allow(clippy::needless_doctest_main)]
//! A Keystore is a secure repository of private keys. KeystoreSender is a
//! reference to a Keystore. KeystoreSender allows async generation of keypairs,
//! and usage of those keypairs, reference by the public AgentHash.
//!
//! # Example
//!
//! ```
//! use holo_hash::AgentHash;
//! use holochain_keystore::*;
//! use holochain_serialized_bytes::prelude::*;
//!
//! #[tokio::main(threaded_scheduler)]
//! async fn main() {
//!     tokio::task::spawn(async move {
//!         let _ = holochain_crypto::crypto_init_sodium();
//!
//!         let keystore = mock_keystore::spawn_mock_keystore(vec![]).await.unwrap();
//!         let agent_hash = AgentHash::new_from_pure_entropy(&keystore).await.unwrap();
//!
//!         #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
//!         struct MyData(Vec<u8>);
//!
//!         let my_data_1 = MyData(b"signature test data 1".to_vec());
//!
//!         let signature = agent_hash.sign(&keystore, &my_data_1).await.unwrap();
//!
//!         assert!(agent_hash.verify_signature(&signature, &my_data_1).await.unwrap());
//!     }).await.unwrap();
//! }
//! ```

use holochain_serialized_bytes::prelude::*;

mod error;
pub use error::*;

mod types;
pub use types::*;

pub mod keystore_actor;
pub use keystore_actor::KeystoreSender;
use keystore_actor::*;

mod agent_hash_ext;
pub use agent_hash_ext::*;

pub mod mock_keystore;
