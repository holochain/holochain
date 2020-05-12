use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::DnaDef;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct InitInvocation {
    // @todo InitWorkspace?
    workspace: UnsafeInvokeZomeWorkspace,
    dna_def: DnaDef,
}

impl Invocation for InitInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::Yes
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        self.dna_def.zomes.keys().cloned().collect()
    }
    fn fn_components(&self) -> FnComponents {
        vec!["init".into()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new(().try_into()?))
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace.clone()
    }
}

impl TryFrom<InitInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(_: InitInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new(().try_into()?))
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

impl From<Vec<InitCallbackResult>> for InitResult {
    fn from(callback_results: Vec<InitCallbackResult>) -> Self {
        callback_results
            .into_iter()
            .fold(Self::Pass, |acc, x| match x {
                InitCallbackResult::Fail(zome_name, fail_string) => {
                    Self::Fail(zome_name, fail_string)
                }
                InitCallbackResult::UnresolvedDependencies(zome_name, ud) => match acc {
                    Self::Fail(_, _) => acc,
                    _ => Self::UnresolvedDependencies(
                        zome_name,
                        ud.into_iter().map(|h| h.into()).collect(),
                    ),
                },
                InitCallbackResult::Pass => Self::Pass,
            })
    }
}
