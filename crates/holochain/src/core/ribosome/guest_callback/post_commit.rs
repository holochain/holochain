use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::header::HeaderHashes;
use holochain_zome_types::post_commit::PostCommitCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct PostCommitInvocation {
    zome_name: ZomeName,
    headers: HeaderHashes,
}

impl Invocation for PostCommitInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::Yes
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        vec![self.zome_name.to_owned()]
    }
    fn fn_components(&self) -> FnComponents {
        vec!["post_commit".into()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.headers).try_into()?))
    }
}

impl TryFrom<PostCommitInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(post_commit_invocation: PostCommitInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new((&post_commit_invocation.headers).try_into()?))
    }
}

pub enum PostCommitResult {
    Success,
    Fail(HeaderHashes, String),
}

impl From<Vec<PostCommitCallbackResult>> for PostCommitResult {
    fn from(callback_results: Vec<PostCommitCallbackResult>) -> Self {
        // this is an optional callback so defaults to success
        callback_results.into_iter().fold(Self::Success, |acc, x| {
            match x {
                // fail overrides everything
                PostCommitCallbackResult::Fail(header_hashes, fail_string) => {
                    Self::Fail(header_hashes, fail_string)
                }
                // success allows acc to continue
                PostCommitCallbackResult::Success => acc,
            }
        })
    }
}

// let mut callback_results: Vec<Option<PostCommitCallbackResult>> = vec![];

// for header in headers {
//     let post_commit_invocation = PostCommitInvocation {
//         zome_name: &zome_name,
//         header: &header,
//     };
//     for callback_output in self.call_iterator(CallbackInvocation::from(post_commit_invocation)) {
//         callback_results.push(match callback_output {
//             Some(implemented) => {
//                 match PostCommitCallbackResult::try_from(implemented.into_inner()) {
//                     // if we deserialize pass straight through
//                     Ok(v) => Some(v),
//                     // if we fail to deserialize this is considered a failure by the happ
//                     // developer to implement the callback correctly
//                     Err(e) => Some(PostCommitCallbackResult::Fail(
//                         header.clone(),
//                         format!("{:?}", e),
//                     )),
//                 }
//             }
//             None => None,
//         });
//         Ok(callback_results)
//     }
// }
//
// // // build all outputs for all callbacks for all headers
// // for header in headers {
// //     let callback_invocation = CallbackInvocation {
// //         components: vec![
// //             "post_commit".into(),
// //             // @todo - if we want to handle entry types we need to decide which ones and
// //             // how/where to construct an enum that represents this as every header type
// //             // is a different struct, and many headers have no associated entry, there is
// //             // no generic way to do something like this pseudocode:
// //             // header.entry_type,
// //         ],
// //         zome_name: zome_name.clone(),
// //         payload: HostInput::new((&header).try_into()?),
// //     };
// //     let callback_outputs: Vec<Option<GuestOutput>> =
// //         self.run_callback(callback_invocation, true)?;
// //     assert_eq!(callback_outputs.len(), 2);
// //
// //     // return the list of results and options so we can log what happened or whatever
// //     // there is no early return of failures because we want to know our response to all
// //     // of the commits
// //     for callback_output in callback_outputs {
// //
// //     }
// // }
