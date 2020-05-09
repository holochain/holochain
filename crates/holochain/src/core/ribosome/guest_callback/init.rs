use holo_hash::EntryHash;
use holochain_zome_types::zome::ZomeName;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::CallbackHostInput;
use crate::core::ribosome::guest_callback::CallbackFnComponents;
use holochain_types::dna::Dna;
use crate::core::ribosome::guest_callback::Invocation;
use crate::core::ribosome::host_fn::AllowSideEffects;
use holochain_zome_types::init::InitCallbackResult;

pub struct InitInvocation {
    dna: Dna
}

impl Invocation for &InitInvocation { }

impl From<&InitInvocation> for AllowSideEffects {
    fn from(invocation: &InitInvocation) -> Self {
        Self::Yes
    }
}

impl From<&InitInvocation> for Vec<ZomeName> {
    fn from(invocation: &InitInvocation) -> Self {
        invocation.dna.zomes.keys().cloned().collect()
    }
}

impl TryFrom<&InitInvocation> for CallbackHostInput {
    type Error = SerializedBytesError;
    fn try_from (_: &InitInvocation) -> Result<Self, Self::Error> {
        Ok(CallbackHostInput::new(().try_into()?))
    }
}

impl From<&InitInvocation> for CallbackFnComponents {
    fn from(_: &InitInvocation) -> Self {
        Self(vec!["init".into()])
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
        callback_results.into_iter().fold(Self::Pass, |acc, x| {
            match x {
                InitCallbackResult::Fail(zome_name, fail_string) => Self::Fail(zome_name, fail_string),
                InitCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Invalid(_) => acc,
                    _ => Self::UnresolvedDependencies(ud),
                },
                InitCallbackResult::Pass => Self::Pass,
            }
        })
    }
}

// let mut init_dna_result = InitDnaResult::Pass;
//
// // we need to init every zome in a dna together, in order
// let zomes = self.dna().zomes.keys();
// for zome_name in zomes {
//     let init_invocation = InitInvocation {
//         dna: self.dna()
//     };
//     let callback_iterator: CallbackIterator<Self> =
//         self.callback_iterator(init_invocation.into());
//
//     let callback_result: Option<CallbackGuestOutput> =
//         match callback_iterator.nth(0) {
//             Some(v) => v,
//             None => unreachable!(),
//         };
//
//     // attempt to deserialize the callback result for this zome
//     init_dna_result = match callback_result {
//         Some(implemented) => match InitCallbackResult::try_from(implemented.into_inner()) {
//             Ok(zome_init_result) => match zome_init_result {
//                 // if this zome passes keep current init dna result
//                 InitCallbackResult::Pass => init_dna_result,
//                 InitCallbackResult::UnresolvedDependencies(entry_hashes) => {
//                     InitDnaResult::UnresolvedDependencies(
//                         zome_name.to_string(),
//                         entry_hashes.into_iter().map(|h| h.into()).collect(),
//                     )
//                 }
//                 // if this zome fails then the dna fails
//                 InitCallbackResult::Fail(fail_string) => {
//                     InitDnaResult::Fail(zome_name.to_string(), fail_string)
//                 }
//             },
//             // failing to deserialize an implemented callback result is a fail
//             Err(e) => InitDnaResult::Fail(zome_name.to_string(), format!("{:?}", e)),
//         },
//         // no init callback for a zome means we keep the current dna state
//         None => init_dna_result,
//     };
//
//     // any fail is a break
//     // continue in the case of unresolved dependencies in case a later zome would fail and
//     // allow us to definitively drop the dna installation
//     match init_dna_result {
//         InitDnaResult::Fail(_, _) => break,
//         _ => {}
//     }
// }
// Ok(init_dna_result)
