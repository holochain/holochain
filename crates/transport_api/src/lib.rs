//! Tokio API for managing low-level transport bindings and connections.
//! # Connection Example
//!
//! ```rust
//! # use transport_api::*;
//! # use futures::future::*;
//! #
//! # pub async fn async_main() {
//! #
//! struct Bob;
//! impl ConnectionHandler for Bob {
//!     fn handle_shutdown(&mut self) -> FutureResult<()> {
//!         async move { Ok(()) }.boxed()
//!     }
//!
//!     fn handle_get_remote_url(&mut self) -> FutureResult<String> {
//!         async move { Ok("test".to_string()) }.boxed()
//!     }
//!
//!     fn handle_outgoing_request(&mut self, data: Vec<u8>) -> FutureResult<Vec<u8>> {
//!         async move { Ok(data) }.boxed()
//!     }
//! }
//! let test_constructor: SpawnConnection<Bob> = Box::new(|_, _| async move { Ok(Bob) }.boxed());
//! let (mut r, _) = spawn_connection(10, test_constructor).await.unwrap();
//! assert_eq!("test", r.get_remote_url().await.unwrap());
//! assert_eq!(b"123".to_vec(), r.outgoing_request(b"123".to_vec()).await.unwrap());
//! #
//! # }
//! #
//! # pub fn main () {
//! #     tokio::runtime::Runtime::new().unwrap().block_on(async_main());
//! # }
//! ```

use thiserror::Error;

/// RpcChannel error type.
#[derive(Error, Debug)]
pub enum TransportError {
    #[error("rpc channel error: {0}")]
    RpcChannel(#[from] rpc_channel::RpcChannelError),

    /// The other end of this channel has been dropped.
    /// No more communication will be possible.
    #[error("channel closed")]
    ChannelClosed,

    /// The handler end dropped the response channel,
    /// you will not receive a response to this request.
    #[error("response channel closed")]
    ResponseChannelClosed,

    /// An unspecified internal error occurred.
    #[error("{0}")]
    Other(String),
}

/// Any for dynamic typing.
pub type BoxAny = Box<dyn ::std::any::Any + 'static + Send>;

/// TransportApi result type.
pub type Result<T> = ::std::result::Result<T, TransportError>;

/// TransportApi async result type.
pub type FutureResult<T> = ::futures::future::BoxFuture<'static, Result<T>>;

mod connection;
pub use connection::*;

mod listener;
pub use listener::*;

#[cfg(test)]
mod tests {
    //use super::*;

    #[tokio::test]
    async fn transport_api_test() {}
}
