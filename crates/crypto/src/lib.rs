#![deny(missing_docs)]

//! holochain_crypto provides cryptographic functions
//!
//! # Example
//!
//! ```
//! # async fn async_main () {
//! use sx_crypto::*;
//!
//! // Make sure to call a system init function!
//! // Otherwise, you'll get `PluginNotInitialized` errors from the api.
//! let _ = crypto_init_sodium();
//!
//! // To sign things, we need to create a keypair.
//! let (mut pub_key, mut sec_key) = crypto_sign_keypair(None).await.unwrap();
//!
//! // Let's just sign a message of 8 zeroes.
//! let mut message = crypto_secure_buffer(8).unwrap();
//!
//! // Get this signature!
//! let mut sig = crypto_sign(&mut message, &mut sec_key).await.unwrap();
//!
//! // Make sure the signature is valid!
//! assert!(crypto_sign_verify(&mut sig, &mut message, &mut pub_key)
//!     .await
//!     .unwrap());
//!
//! // Let's tweak the signature so it becomes invalid.
//! {
//!     let mut sig = sig.write();
//!     sig[0] = (std::num::Wrapping(sig[0]) + std::num::Wrapping(1)).0;
//! }
//!
//! // Make sure the invalid signature is invalid!
//! assert!(!crypto_sign_verify(&mut sig, &mut message, &mut pub_key)
//!     .await
//!     .unwrap());
//! # }
//! # fn main () {
//! #     tokio::runtime::Builder::new().threaded_scheduler()
//! #         .build().unwrap().block_on(async move {
//! #             tokio::task::spawn(async_main()).await
//! #         });
//! # }
//! ```

use rust_sodium_holochain_fork_sys as rust_sodium_sys;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use futures::future::{BoxFuture, FutureExt};

#[macro_use]
mod macros;

mod error;
pub use error::*;

mod bytes;
pub use bytes::*;

pub mod plugin;

mod sodium;
pub use sodium::*;

mod api;
pub use api::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(threaded_scheduler)]
    async fn sodium_randombytes_buf() {
        let _ = crypto_init_sodium();
        tokio::task::spawn(async move {
            let mut buf = crypto_secure_buffer(8).unwrap();
            assert_eq!(
                "[0, 0, 0, 0, 0, 0, 0, 0]",
                &format!("{:?}", buf.read().deref()),
            );

            crypto_randombytes_buf(&mut buf).await.unwrap();
            assert_ne!(
                "[0, 0, 0, 0, 0, 0, 0, 0]",
                &format!("{:?}", buf.read().deref()),
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn sodium_generic_hash() {
        let _ = crypto_init_sodium();
        tokio::task::spawn(async move {
            let mut buf = crypto_secure_buffer(8).unwrap();
            assert_eq!(
                "[0, 0, 0, 0, 0, 0, 0, 0]",
                &format!("{:?}", buf.read().deref()),
            );

            let hash = crypto_generic_hash(16, &mut buf, None).await.unwrap();
            assert_eq!(
                "[200, 4, 206, 25, 142, 195, 55, 227, 220, 118, 43, 221, 26, 9, 174, 206]",
                &format!("{:?}", hash.read().deref()),
            );
        })
        .await
        .unwrap();
    }

    #[tokio::test(threaded_scheduler)]
    async fn sodium_sign_no_seed() {
        let _ = crypto_init_sodium();
        tokio::task::spawn(async move {
            let mut message = crypto_secure_buffer(8).unwrap();
            let (mut pub_key, mut sec_key) = crypto_sign_keypair(None).await.unwrap();
            let mut sig = crypto_sign(&mut message, &mut sec_key).await.unwrap();

            assert!(crypto_sign_verify(&mut sig, &mut message, &mut pub_key)
                .await
                .unwrap());

            {
                let mut sig = sig.write();
                sig[0] = (std::num::Wrapping(sig[0]) + std::num::Wrapping(1)).0;
            }

            assert!(!crypto_sign_verify(&mut sig, &mut message, &mut pub_key)
                .await
                .unwrap());
        })
        .await
        .unwrap();
    }
}
