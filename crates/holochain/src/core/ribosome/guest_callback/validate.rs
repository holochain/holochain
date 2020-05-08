use crate::core::ribosome::guest_callback::AllowSideEffects;
use crate::core::ribosome::guest_callback::CallbackFnComponents;
use holochain_zome_types::entry::Entry;
use crate::core::ribosome::guest_callback::Invocation;
use holochain_types::nucleus::ZomeName;
use holochain_zome_types::CallbackHostInput;
use holochain_serialized_bytes::prelude::*;

pub struct ValidateInvocation<'a> {
    zome_name: ZomeName,
    entry: &'a Entry,
}

impl Invocation for &ValidateInvocation<'_> { }

impl From<&ValidateInvocation<'_>> for ZomeName {
    fn from(validate_invocation: &ValidateInvocation) -> ZomeName {
        validate_invocation.zome_name.clone()
    }
}

impl TryFrom<&ValidateInvocation<'_>> for CallbackHostInput {
    type Error = SerializedBytesError;
    fn try_from(validate_invocation: &ValidateInvocation) -> Result<Self, Self::Error> {
        Ok(CallbackHostInput::new(validate_invocation.entry.try_into()?))
    }
}

impl From<&ValidateInvocation<'_>> for CallbackFnComponents {
    fn from(validate_invocation: &ValidateInvocation) -> CallbackFnComponents {
        CallbackFnComponents(vec![
            "validate".into(),
            match validate_invocation.entry {
                Entry::Agent(_) => "agent",
                Entry::App(_) => "entry",
                Entry::CapTokenClaim(_) => "cap_token_claim",
                Entry::CapTokenGrant(_) => "cap_token_grant",
            }.into(),
        ])
    }
}

impl From<&ValidateInvocation<'_>> for AllowSideEffects {
    fn from(_: &ValidateInvocation) -> AllowSideEffects {
        AllowSideEffects::No
    }
}

// let callback_invocation = CallbackInvocation {
//     components: vec![
//         "validate".into(),
//         match entry {
//             Entry::Agent(_) => "agent",
//             Entry::App(_) => "entry",
//             Entry::CapTokenClaim(_) => "cap_token_claim",
//             Entry::CapTokenGrant(_) => "cap_token_grant",
//         }
//         .into(),
//     ],
//     zome_name,
//     payload: CallbackHostInput::new(entry.try_into()?),
// };
// let callback_outputs: Vec<Option<CallbackGuestOutput>> =
//     self.run_callback(callback_invocation, false)?;
// assert_eq!(
//     callback_outputs.len(),
//     2,
//     "validate had wrong number of callbacks"
// );

// for callback_outputs in self.callback_iterator(CallbackInvocation::from(ValidateInvocation {
//     zome_name: &zome_name,
//     entry,
// })) {
//     Ok(callback_outputs
//         .into_iter()
//         .map(|r| match r {
//             Some(implemented) => {
//                 match ValidateCallbackResult::try_from(implemented.into_inner()) {
//                     Ok(v) => v,
//                     // failing to inflate is an invalid result
//                     Err(e) => ValidateCallbackResult::Invalid(format!("{:?}", e)),
//             }
//             // not implemented = valid
//             // note that if NO callbacks are implemented we always pass validation
//             None => ValidateCallbackResult::Valid,
//         })
//         // folded into a single validation result
//         .fold(ValidateEntryResult::Valid, |acc, x| {
//             match x {
//                 // validation is invalid if any x is invalid
//                 ValidateCallbackResult::Invalid(i) => ValidateEntryResult::Invalid(i),
//                 // return unresolved dependencies if it's otherwise valid
//                 ValidateCallbackResult::UnresolvedDependencies(ud) => match acc {
//                     ValidateEntryResult::Invalid(_) => acc,
//                     _ => ValidateEntryResult::UnresolvedDependencies(ud),
//                 },
//                 // valid x allows validation to continue
//                 ValidateCallbackResult::Valid => acc,
//             }
//         }))
//     }
