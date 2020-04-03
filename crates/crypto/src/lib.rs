#![deny(missing_docs)]

//! holochain_crypto provides cryptographic functions

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
}
