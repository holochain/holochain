use thiserror::Error;

/// RichChannel error type.
#[derive(Error, Debug)]
pub enum RichChannelError {
    /// The other end of this channel has been dropped.
    /// No more communication will be possible.
    #[error("channel closed")]
    ChannelClosed,

    /// The handler end dropped the response channel,
    /// you will not receive a response to this request.
    #[error("response channel closed")]
    ResponseChannelClosed,

    /// An unspecified internal error occurred.
    #[error("{0:?}")]
    Other(String),
}

/// RichChannel result type.
pub type Result<T> = ::std::result::Result<T, RichChannelError>;

/// Trait indicating a type designed to be sent through a rich_channel.
pub trait RichChannelMsg: 'static + Send {
    /// The response type associated with this message type.
    /// Pro Tip: This can optionally be set to the Self type.
    type ResponseType: 'static + Send;
}

/// The "sender" side of a rich_channel.
pub struct RichChannelSender<T: RichChannelMsg> {
    sender: tokio::sync::mpsc::Sender<(T, tokio::sync::oneshot::Sender<Result<T::ResponseType>>)>,
}

// not sure why derive(Clone) doesn't work here
impl<T: RichChannelMsg> Clone for RichChannelSender<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T: RichChannelMsg> RichChannelSender<T> {
    /// The request function on a RichChannelSender.
    pub async fn request(&mut self, data: T) -> Result<T::ResponseType> {
        let (one_send, one_recv) = tokio::sync::oneshot::channel();
        self.sender.send((data, one_send)).await.map_err(|_|RichChannelError::ChannelClosed)?;
        let resp = one_recv.await.map_err(|_|RichChannelError::ResponseChannelClosed)?;
        resp
    }
}

/// Handler callback for servicing RichChannelReceiver "recv" operations.
/// Note this callback is not "async" - though you can choose to return
/// futures in your response type.
pub type RichChannelHandler<'lt, T, R> = Box<dyn FnMut(T) -> Result<R> + 'lt + Send>;

/// The "receiver" side of a rich_channel.
pub struct RichChannelReceiver<T: RichChannelMsg> {
    receiver: tokio::sync::mpsc::Receiver<(T, tokio::sync::oneshot::Sender<Result<T::ResponseType>>)>,
}

impl<T: RichChannelMsg> RichChannelReceiver<T> {
    /// Handle any incoming messages by invoking the "handler" callback.
    /// Will return an error if the channel is broken.
    pub async fn recv<'a>(
        &'a mut self,
        handler: &mut RichChannelHandler<'a, T, T::ResponseType>,
    ) -> Result<()> {
        let (data, respond) = match self.receiver.recv().await {
            None => Err(RichChannelError::ChannelClosed),
            Some(r) => Ok(r),
        }?;
        let result = handler(data);
        if let Err(_) = respond.send(result) {
            return Err(RichChannelError::ResponseChannelClosed);
        }
        Ok(())
    }
}

/// Create a new rich_channel.
pub fn rich_channel<T: RichChannelMsg>(channel_size: usize) ->
    (RichChannelSender<T>, RichChannelReceiver<T>)
{
    let (sender, receiver) = tokio::sync::mpsc::channel(channel_size);
    (
        RichChannelSender {
            sender,
        },
        RichChannelReceiver {
            receiver,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rich_channel_send_recv() {
        struct Msg(String);
        impl RichChannelMsg for Msg {
            type ResponseType = Msg;
        }

        let (mut send, mut recv) = rich_channel::<Msg>(10);

        tokio::task::spawn(async move {
            let mut handler: RichChannelHandler<Msg, Msg> = Box::new(|data| {
                Ok(Msg(format!("{} world", data.0)))
            });
            recv.recv(&mut handler).await.unwrap();
        });

        let res = send.request(Msg("hello".to_string())).await.unwrap();
        assert_eq!("hello world", &res.0);
    }
}
