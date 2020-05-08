use holochain_types::header::AppEntryType;

pub struct ValidationPackageInvocation<'a> {
    zome_name: &'a str,
    app_entry_type: &'a AppEntryType,
}

// // let callback_invocation = CallbackInvocation {
// //     components: vec![
// //         "custom_validation_package".into(),
// //         // @todo zome_id is a u8, is this really an ergonomic way for us to interact with
// //         // entry types at the happ code level?
// //         format!("{}", app_entry_type.zome_id()),
// //     ],
// //     zome_name: zome_name.clone(),
// //     payload: CallbackHostInput::new(app_entry_type.try_into()?),
// // };
// // let mut callback_outputs: Vec<Option<CallbackGuestOutput>> =
// //     self.run_callback(callback_invocation, false)?;
// // assert_eq!(callback_outputs.len(), 2);
//
// let validation_package_invocation = ValidationPackageInvocation {
//     zome_name,
//     app_entry_type,
// };
//
// // we only keep the most specific implemented package, if it exists
// // note this means that if zome devs are ambiguous about their implementations it could
// // lead to redundant work, but this is an edge case easily avoided by a happ dev and hard
// // for us to guard against, so we leave that thinking up to the implementation
// match self.callback_iterator(validation_package_invocation.into()).nth(0) {
//     Some(implemented) => {
//         match ValidationPackageCallbackResult::try_from(implemented?.into_inner()) {
//             // if we manage to deserialize a package nicely we return it
//             Ok(v) => v,
//             // if we can't deserialize the package, that's a fail
//             Err(e) => ValidationPackageCallbackResult::Fail(format!("{:?}", e)),
//         }
//     },
//     // a missing validation package callback for a specific app entry type and zome is a
//     // fail because this callback should only be triggered _if we know we need package_
//     // because core has already decided that the default subconscious packages are not
//     // sufficient
//     None => ValidationPackageCallbackResult::Fail(format!(
//         "Missing validation package callback for entry type: {:?} in zome {:?}",
//         &app_entry_type, &zome_name
//     )),
// }
