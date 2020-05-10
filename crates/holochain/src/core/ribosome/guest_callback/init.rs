use holo_hash::EntryHash;
use holochain_zome_types::zome::ZomeName;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::HostInput;
use crate::core::ribosome::FnComponents;
use holochain_types::dna::Dna;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::AllowSideEffects;
use holochain_zome_types::init::InitCallbackResult;

#[derive(Clone)]
pub struct InitInvocation {
    dna: Dna
}

impl Invocation for InitInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::Yes
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        self.dna.zomes.keys().cloned().collect()
    }
    fn fn_components(&self) -> FnComponents {
        vec!["init".into()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new(().try_into()?))
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
        callback_results.into_iter().fold(Self::Pass, |acc, x| {
            match x {
                InitCallbackResult::Fail(zome_name, fail_string) => Self::Fail(zome_name, fail_string),
                InitCallbackResult::UnresolvedDependencies(zome_name, ud) => match acc {
                    Self::Fail(_, _) => acc,
                    _ => Self::UnresolvedDependencies(zome_name, ud.into_iter().map(|h| h.into()).collect()),
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
//     let call_iterator: CallIterator<Self> =
//         self.call_iterator(init_invocation.into());
//
//     let callback_result: Option<GuestOutput> =
//         match call_iterator.nth(0) {
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
