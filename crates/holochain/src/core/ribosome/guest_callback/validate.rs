use crate::core::ribosome::guest_callback::AllowSideEffects;
use crate::core::ribosome::guest_callback::CallbackFnComponents;
use holochain_zome_types::entry::Entry;

pub struct ValidateInvocation<'a> {
    zome_name: &'a str,
    entry: &'a Entry,
}

impl From<&ValidateInvocation<'_>> for CallbackFnComponents {
    fn from(validate_invocation: &ValidateInvocation) -> CallbackFnComponents {
        CallbackFnComponents(vec![
            "validate",
            match validate_invocation.entry {
                Entry::Agent(_) => "agent",
                Entry::App(_) => "entry",
                Entry::CapTokenClaim(_) => "cap_token_claim",
                Entry::CapTokenGrant(_) => "cap_token_grant",
            },
        ])
    }
}

impl From<&ValidateInvocation<'_>> for AllowSideEffects {
    fn from(_: &ValidateInvocation) -> AllowSideEffects {
        AllowSideEffects::No
    }
}
