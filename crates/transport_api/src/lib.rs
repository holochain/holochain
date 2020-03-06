//! Tokio API for managing low-level transport bindings and connections.
//! # Connection Example
//!
//! ```rust
//! # use transport_api::*;
//! # use futures::future::*;
//! # use url2::prelude::*;
//! #
//! # pub async fn async_main() {
//! #
//! struct MyListener;
//! impl ListenerHandler for MyListener {
//!     // ...
//! #     fn handle_shutdown(&mut self) -> FutureResult<()> {
//! #         async move { Ok(()) }.boxed()
//! #     }
//! #
//! #     fn handle_get_bound_url(&mut self) -> FutureResult<Url2> {
//! #         async move { Ok(url2!("test://test/")) }.boxed()
//! #     }
//! #
//! #     fn handle_connect(
//! #         &mut self,
//! #         _url: Url2,
//! #     ) -> FutureResult<(ConnectionSender, IncomingRequestReceiver)> {
//! #         async move { Err(TransportError::Other("unimplemented".into())) }.boxed()
//! #     }
//! }
//!
//! struct MyConnection;
//! impl ConnectionHandler for MyConnection {
//!     // ...
//! #     fn handle_shutdown(&mut self) -> FutureResult<()> {
//! #         async move { Ok(()) }.boxed()
//! #     }
//!
//!     fn handle_get_remote_url(&mut self) -> FutureResult<Url2> {
//!         async move { Ok(url2!("test://test/")) }.boxed()
//!     }
//!
//!     fn handle_outgoing_request(&mut self, data: Vec<u8>) -> FutureResult<Vec<u8>> {
//!         async move { Ok(data) }.boxed()
//!     }
//! }
//!
//! let (listener, _) = spawn_listener(10, "test", |_, _| {
//!     async move { Ok(MyListener) }.boxed()
//! }).await.unwrap();
//!
//! let (mut con, _) = spawn_connection(10, listener, |_, _| {
//!     async move { Ok(MyConnection) }.boxed()
//! }).await.unwrap();
//!
//! assert_eq!("test://test/", con.get_remote_url().await.unwrap().as_str());
//! assert_eq!(b"123".to_vec(), con.outgoing_request(b"123".to_vec()).await.unwrap());
//! #
//! # }
//! #
//! # pub fn main () {
//! #     tokio::runtime::Runtime::new().unwrap().block_on(async_main());
//! # }
//! ```

use thiserror::Error;
use url2::prelude::*;

/// TransportApi error type.
#[derive(Error, Debug)]
pub enum TransportError {
    #[error("rpc channel error: {0}")]
    RpcChannel(#[from] rpc_channel::RpcChannelError),

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

mod transport_pool;
pub use transport_pool::*;
