use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::sync::Arc;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;

#[derive(Clone)]
pub struct ValidateInvocation {
    // @todo ValidateWorkspace?
    pub workspace: UnsafeInvokeZomeWorkspace,
    pub zome_name: ZomeName,
    // Arc here as entry may be very large
    // don't want to clone the Entry just to validate it
    // we can SerializedBytes off an Entry reference
    // lifetimes on invocations are a pain
    pub entry: Arc<Entry>,
}

impl Invocation for ValidateInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::No
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        // entries are specific to zomes so only validate in the zome the entry is defined in
        // note that here it is possible there is a zome/entry mismatch
        // we rely on the invocation to be built correctly
        vec![self.zome_name.clone()]
    }
    fn fn_components(&self) -> FnComponents {
        vec![
            "validate".into(),
            match *self.entry {
                Entry::Agent(_) => "agent",
                Entry::App(_) => "entry",
                Entry::CapTokenClaim(_) => "cap_token_claim",
                Entry::CapTokenGrant(_) => "cap_token_grant",
            }
            .into(),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&*self.entry).try_into()?))
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace
    }
}

impl TryFrom<ValidateInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(validate_invocation: ValidateInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new((&*validate_invocation.entry).try_into()?))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidateResult {
    Valid,
    Invalid(String),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

impl From<Vec<ValidateCallbackResult>> for ValidateResult {
    fn from(callback_results: Vec<ValidateCallbackResult>) -> Self {
        callback_results.into_iter().fold(Self::Valid, |acc, x| {
            match x {
                // validation is invalid if any x is invalid
                ValidateCallbackResult::Invalid(i) => Self::Invalid(i),
                // return unresolved dependencies if it's otherwise valid
                ValidateCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Invalid(_) => acc,
                    _ => Self::UnresolvedDependencies(ud.into_iter().map(|h| h.into()).collect()),
                },
                // valid x allows validation to continue
                ValidateCallbackResult::Valid => acc,
            }
        })
    }
}
