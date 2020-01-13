use crossbeam_channel::{Receiver, SendError, Sender};
use futures::task::Poll;
use futures::Future;

// pub fn request<Req: Send, Res: Send>(
//     payload: Req,
// ) -> (ChanRequest<Req, Res>, ChanResponseFuture<Res>) {
//     ChanRequest::new(payload)
// }

#[derive(Shrinkwrap)]
pub struct ChanRequest<Req, Res> {
    #[shrinkwrap(main_field)]
    payload: Req,
    tx_response: Sender<ChanResponse<Res>>,
}

#[derive(Shrinkwrap)]
pub struct ChanResponse<Res> {
    #[shrinkwrap(main_field)]
    payload: Res,
}

impl<Req: Send, Res: Send> ChanRequest<Req, Res> {
    pub fn new(payload: Req) -> (Self, ChanResponseFuture<Res>) {
        let (tx_response, rx_response) = crossbeam_channel::bounded(0);
        let req = Self {
            payload,
            tx_response,
        };
        let res = ChanResponseFuture::new(rx_response);
        (req, res)
    }
}

impl<Req: Send, Res: Send> Request<Req, Res, ChanResponse<Res>> for ChanRequest<Req, Res> {

    fn respond(self, payload: Res) -> Result<(), SendError<ChanResponse<Res>>> {
        let request_id = self.request_id.clone();
        self.respond_raw(ChanResponse {
            payload,
            request_id,
        })
    }

    fn respond_raw(self, response: ChanResponse<Res>) -> Result<(), SendError<ChanResponse<Res>>> {
        self.tx_response.send(response)
    }
}

impl<Res> Response<Res> for ChanResponse<Res> {}

/// Wait for a response to be sent on a pre-established channel
/// (Don't know what I'm doing here, just trying stuff)
pub struct ChanResponseFuture<Res> {
    rx: Receiver<ChanResponse<Res>>,
}

impl<Res> ChanResponseFuture<Res> {
    pub fn new(rx: Receiver<ChanResponse<Res>>) -> Self {
        Self { rx }
    }
}

impl<Res> Future for ChanResponseFuture<Res> {
    type Output = ChanResponse<Res>;

    fn poll(self: std::pin::Pin<&mut Self>, ctx: &mut std::task::Context) -> Poll<Self::Output> {
        if let Ok(val) = self.rx.try_recv() {
            Poll::Ready(val)
        } else {
            ctx.waker().clone().wake();
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod test {

}