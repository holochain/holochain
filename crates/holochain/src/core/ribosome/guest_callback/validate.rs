use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use holochain_zome_types::entry::Entry;
use crate::core::ribosome::Invocation;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use holochain_zome_types::validate::ValidateCallbackResult;
use holochain_serialized_bytes::prelude::*;
use std::sync::Arc;
use holo_hash::EntryHash;

#[derive(Clone)]
pub struct ValidateInvocation {
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
            }.into(),
            ].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&*self.entry).try_into()?))
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
//     payload: HostInput::new(entry.try_into()?),
// };
// let callback_outputs: Vec<Option<GuestOutput>> =
//     self.run_callback(callback_invocation, false)?;
// assert_eq!(
//     callback_outputs.len(),
//     2,
//     "validate had wrong number of callbacks"
// );

// for callback_outputs in self.call_iterator(CallbackInvocation::from(ValidateInvocation {
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
