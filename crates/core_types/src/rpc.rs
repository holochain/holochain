//! Patterns for making channel-based RPC calls and awaiting for the response via a future.
//!
//! ```
//! use std::thread;
//!
//! let (req, res) = skunkworx_core_types::rpc::Request::new("hello".into());
//! thread::spawn(move || {
//!     let response = format!("{}!!!", req);
//!     req.respond(response);
//! });
//!
//! ```


use crossbeam_channel::{Receiver, SendError, Sender};
use futures::task::Poll;
use futures::Future;
use snowflake::ProcessUniqueId;

pub fn request<Req: Send, Res: Send>(
    payload: Req,
) -> (Request<Req, Res>, ResponseFuture<Res>) {
    Request::new(payload)
}

#[derive(Shrinkwrap)]
pub struct Request<Req, Res> {
    #[shrinkwrap(main_field)]
    payload: Req,
    request_id: ProcessUniqueId,
    tx_response: Sender<Response<Res>>,
}

#[derive(Shrinkwrap)]
pub struct Response<Res> {
    #[shrinkwrap(main_field)]
    payload: Res,
    request_id: ProcessUniqueId,
}

impl<Req: Send, Res: Send> Request<Req, Res> {
    pub fn new(payload: Req) -> (Self, ResponseFuture<Res>) {
        let (tx_response, rx_response) = crossbeam_channel::bounded(0);
        let req = Self {
            request_id: ProcessUniqueId::new(),
            payload,
            tx_response,
        };
        let res = ResponseFuture::new(rx_response);
        (req, res)
    }

    pub fn respond(self, payload: Res) -> Result<(), SendError<Response<Res>>> {
        let request_id = self.request_id.clone();
        self.respond_raw(Response {
            payload,
            request_id,
        })
    }

    fn respond_raw(self, response: Response<Res>) -> Result<(), SendError<Response<Res>>> {
        self.tx_response.send(response)
    }
}

/// Wait for a response to be sent on a pre-established channel
/// (Don't know what I'm doing here, just trying stuff)
pub struct ResponseFuture<Res> {
    rx: Receiver<Response<Res>>,
}

impl<Res> ResponseFuture<Res> {
    pub fn new(rx: Receiver<Response<Res>>) -> Self {
        Self { rx }
    }
}

impl<Res> Future for ResponseFuture<Res> {
    type Output = Response<Res>;

    fn poll(self: std::pin::Pin<&mut Self>, ctx: &mut std::task::Context) -> Poll<Self::Output> {
        if let Ok(val) = self.rx.try_recv() {
            Poll::Ready(val)
        } else {
            ctx.waker().clone().wake();
            Poll::Pending
        }
    }
}

/////////////////////////////////////////////////////////////////

use std::collections::HashMap;

type TxResponse<T> = Sender<Response<T>>;
type RxResponse<T> = Receiver<Response<T>>;

struct RpcSystem<Payload> {
    pending: HashMap<ProcessUniqueId, Request<Payload, Payload>>,
}

impl<Payload: Send> RpcSystem<Payload> {

    fn new() -> Self {
        Self {
            pending: HashMap::new()
        }
    }

    pub fn create() -> (impl Future<Output=()>, TxResponse<Payload>) {
        let system = Self::new();
        let (tx_response, rx_response) = crossbeam_channel::bounded(1);
        let fut = system.run(rx_response);
        (fut, tx_response)
    }

    async fn run(mut self, rx_response: Receiver<Response<Payload>>) {
        loop {
            if let Ok(response) = rx_response.try_recv() {
                let request_id = &response.request_id;
                if let Some(request) = self.pending.remove(request_id) {
                    request.respond_raw(response);
                } else {
                    warn!("Received a response for which there was no request! request_id={}", request_id);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {

}