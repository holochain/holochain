use tokio::sync::oneshot;
use tokio::task::AbortHandle;

pub(crate) struct AbortOnDropHandle(AbortHandle);

impl AbortOnDropHandle {
    pub fn new(abort_handle: AbortHandle) -> Self {
        Self(abort_handle)
    }
}

impl Drop for AbortOnDropHandle {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Resolves when a websocket connection is no longer usable.
///
/// The client polls each websocket on a background task. That task ends when
/// the connection drops, which is what this notification reports.
pub struct ClosedNotify(oneshot::Receiver<()>);

impl ClosedNotify {
    pub(crate) fn new(rx: oneshot::Receiver<()>) -> Self {
        Self(rx)
    }

    /// Waits until the connection is no longer usable.
    pub async fn closed(self) {
        // An error means the poll task was aborted because the websocket was
        // dropped, which is equally a reason to stop using the connection.
        let _ = self.0.await;
    }
}
