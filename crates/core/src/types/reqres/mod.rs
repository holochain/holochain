//! Patterns for making channel-based RPC calls and awaiting for the response via a future.
//!
//! ```
//! use std::thread;
//!
//! let (req, res) = sx_core::rpc::RpcRequest::new("hello".into());
//! self.network_send(req).unwrap();
//! let response = res.await;
//!
//!
//! let req = "hello";
//! let response = self.network_send(req.into()).await.unwrap();
//!
//! ```

use crossbeam_channel::SendError;

pub mod chan;
pub mod rpc;

pub trait Request<ReqPayload, ResPayload, Res: Response<ResPayload>> {
    fn respond(self, payload: ResPayload) -> Result<(), SendError<Res>>;
    fn respond_raw(self, response: Res) -> Result<(), SendError<Res>>;
}

pub trait Response<ResPayload> {}
