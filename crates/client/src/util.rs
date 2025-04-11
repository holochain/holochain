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
