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

    #[test]
    fn double_check_same_blake2b_hash() {
        assert_eq!(
            "[200, 4, 206, 25, 142, 195, 55, 227, 220, 118, 43, 221, 26, 9, 174, 206]",
            &format!(
                "{:?}",
                blake2b_simd::Params::new()
                    .hash_length(16)
                    .hash(&vec![0; 8])
                    .as_bytes(),
            ),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn sodium_dht_location() {
        let _ = crypto_init_sodium();
        tokio::task::spawn(async move {
            let mut buf = crypto_secure_buffer(8).unwrap();
            assert_eq!(
                "[0, 0, 0, 0, 0, 0, 0, 0]",
                &format!("{:?}", buf.read().deref()),
            );

            let loc = crypto_dht_location(&mut buf).await.unwrap();
            assert_eq!(3917265024, loc);
        })
        .await
        .unwrap();
    }

    #[test]
    fn double_check_same_blake2b_loc() {
        let hash = blake2b_simd::Params::new()
            .hash_length(16)
            .hash(&vec![0; 8]);
        let hash = hash.as_bytes();
        let mut out: [u8; 4] = [hash[0], hash[1], hash[2], hash[3]];
        for i in (4..16).step_by(4) {
            out[0] ^= hash[i];
            out[1] ^= hash[i + 1];
            out[2] ^= hash[i + 2];
            out[3] ^= hash[i + 3];
        }
        let loc = (out[0] as u32)
            + ((out[1] as u32) << 8)
            + ((out[2] as u32) << 16)
            + ((out[3] as u32) << 24);
        assert_eq!(3917265024, loc);
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
