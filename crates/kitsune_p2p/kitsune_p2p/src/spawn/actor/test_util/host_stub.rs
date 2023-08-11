use crate::event::{KitsuneP2pEvent, PutAgentInfoSignedEvt};
use crate::KitsuneP2pError;
use futures::channel::mpsc::{channel, Receiver};
use futures::{FutureExt, SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::AbortHandle;
use tokio::time::error::Elapsed;

pub struct HostStub {
    pub respond_with_error: Arc<AtomicBool>,
    put_events: Receiver<PutAgentInfoSignedEvt>,
    abort_handle: AbortHandle,
}

impl HostStub {
    pub fn start(mut host_receiver: Receiver<KitsuneP2pEvent>) -> Self {
        let (mut sender, receiver) = channel(10);

        let respond_with_error = Arc::new(AtomicBool::new(false));
        let handle = tokio::spawn({
            let task_respond_with_error = respond_with_error.clone();
            async move {
                while let Some(evt) = host_receiver.next().await {
                    match evt {
                        KitsuneP2pEvent::PutAgentInfoSigned { input, respond, .. } => {
                            if task_respond_with_error.load(Ordering::SeqCst) {
                                respond.respond(Ok(async move {
                                    Err(KitsuneP2pError::other("a test error"))
                                }
                                .boxed()
                                .into()));
                                continue;
                            }

                            sender.send(input).await.unwrap();
                            respond.respond(Ok(async move { Ok(()) }.boxed().into()));
                        }
                        _ => panic!("Unexpected event - {:?}", evt),
                    }
                }
            }
        });

        HostStub {
            respond_with_error,
            put_events: receiver,
            abort_handle: handle.abort_handle(),
        }
    }

    pub async fn next_event(&mut self, timeout: Duration) -> PutAgentInfoSignedEvt {
        tokio::time::timeout(timeout, self.put_events.next())
            .await
            .unwrap()
            .unwrap()
    }

    pub async fn try_next_event(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<PutAgentInfoSignedEvt>, Elapsed> {
        tokio::time::timeout(timeout, self.put_events.next()).await
    }

    pub fn abort(&self) {
        self.abort_handle.abort();
    }
}
