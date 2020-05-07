use crate::core::ribosome::guest_callback::CallbackInvocation;
use holochain_types::dna::Dna;

pub struct InitInvocation<'a> {
    dna: &'a Dna,
}

impl From<InitInvocation<'_>> for CallbackInvocation<'_> {
    fn from(init_invocation: InitInvocation<'_>) -> Self {
        Self::Init(init_invocation)
    }
}

/// the aggregate result of _all_ init callbacks
pub enum InitResult {
    /// all init callbacks passed
    Pass,
    /// some init failed
    /// ZomeName is the first zome that failed to init
    /// String is a human-readable error string giving the reason for failure
    Fail(ZomeName, String),
    /// no init failed but some zome has unresolved dependencies
    /// ZomeName is the first zome that has unresolved dependencies
    /// Vec<EntryHash> is the list of all missing dependency addresses
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
}
