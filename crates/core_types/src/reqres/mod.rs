//! Patterns for making channel-based RPC calls and awaiting for the response via a future.
//!
//! ```
//! use std::thread;
//!
//! let (req, res) = skunkworx_core_types::rpc::RpcRequest::new("hello".into());
//! thread::spawn(move || {
//!     let response = format!("{}!!!", req);
//!     req.respond(response);
//! });
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
