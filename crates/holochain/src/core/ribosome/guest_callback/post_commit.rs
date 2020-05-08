use holo_hash::HeaderHash;

pub struct PostCommitInvocation<'a> {
    zome_name: &'a str,
    header: &'a HeaderHash,
}

pub struct PostCommitResult;


        // let mut callback_results: Vec<Option<PostCommitCallbackResult>> = vec![];

        // for header in headers {
        //     let post_commit_invocation = PostCommitInvocation {
        //         zome_name: &zome_name,
        //         header: &header,
        //     };
        //     for callback_output in self.callback_iterator(CallbackInvocation::from(post_commit_invocation)) {
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
        // //         payload: CallbackHostInput::new((&header).try_into()?),
        // //     };
        // //     let callback_outputs: Vec<Option<CallbackGuestOutput>> =
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
