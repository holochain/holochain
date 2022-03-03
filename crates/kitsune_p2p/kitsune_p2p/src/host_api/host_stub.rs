use crate::KitsuneHostPanicky;

/// Dummy host impl for plumbing
pub struct HostStub;

impl KitsuneHostPanicky for HostStub {
    const NAME: &'static str = "HostStub";
}

impl HostStub {
    /// Constructor
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
}
