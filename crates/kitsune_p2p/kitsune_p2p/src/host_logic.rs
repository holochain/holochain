use kitsune_p2p_types::bin_types::KitsuneSpace;

/// The interface to be implemented by the host, which handles various requests
/// for data
pub trait KitsuneHost {
    /// Dummy function
    fn get_foo(&self, space: &KitsuneSpace) -> !;
}

/// Trait object for the host interface
pub type HostApi = std::sync::Arc<dyn KitsuneHost + Send + Sync>;

/// Dummy host impl for plumbing
pub struct HostStub;

impl KitsuneHost for HostStub {
    fn get_foo(&self, _space: &KitsuneSpace) -> ! {
        todo!()
    }
}

impl HostStub {
    /// Constructor
    pub fn new() -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self)
    }
}
