use kitsune_p2p_fetch::FetchQueueConfig;

use crate::KitsuneHostDefaultError;

/// Dummy host impl for plumbing
pub struct HostStub;

impl KitsuneHostDefaultError for HostStub {
    const NAME: &'static str = "HostStub";
}

impl HostStub {
    /// Constructor
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
}

impl FetchQueueConfig for HostStub {
    fn merge_fetch_contexts(&self, _a: u32, _b: u32) -> u32 {
        unimplemented!()
    }
}
