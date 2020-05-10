use holochain_types::header::AppEntryType;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::validate::ValidationPackage;
use holo_hash::EntryHash;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::HostInput;
use holochain_zome_types::validate::ValidationPackageCallbackResult;

#[derive(Clone)]
pub struct ValidationPackageInvocation {
    zome_name: ZomeName,
    app_entry_type: AppEntryType,
}

impl Invocation for ValidationPackageInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::No
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        vec![self.zome_name.to_owned()]
    }
    fn fn_components(&self) -> FnComponents {
        // @todo zome_id is a u8, is this really an ergonomic way for us to interact with
        // entry types at the happ code level?
        vec!["validation_package".into(), format!("{}", self.app_entry_type.zome_id())].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.app_entry_type).try_into()?))
    }
}

impl TryFrom<ValidationPackageInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(validation_package_invocation: ValidationPackageInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new((&validation_package_invocation.app_entry_type).try_into()?))
    }
}


pub enum ValidationPackageResult {
    Success(ValidationPackage),
    Fail(ZomeName, String),
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
    NotImplemented,
}

impl From<Vec<ValidationPackageCallbackResult>> for ValidationPackageResult {
    fn from(callback_results: Vec<ValidationPackageCallbackResult>) -> Self {
        // the default situation is a special case that nothing was implemented
        // upstream will likely want to handle this case explicitly
        callback_results.into_iter().fold(Self::NotImplemented, |acc, x| {
            match x {
                ValidationPackageCallbackResult::Fail(zome_name, fail_string) => Self::Fail(zome_name, fail_string),
                ValidationPackageCallbackResult::UnresolvedDependencies(zome_name, ud) => match acc {
                    // failure anywhere overrides unresolved deps
                    Self::Fail(_, _) => acc,
                    // unresolved deps overrides anything other than failure
                    _ => Self::UnresolvedDependencies(zome_name, ud.into_iter().map(|h| h.into()).collect()),
                },
                ValidationPackageCallbackResult::Success(package) => match acc {
                    // fail anywhere overrides success
                    Self::Fail(_, _) => acc,
                    // unresolved deps anywhere overrides success anywhere
                    Self::UnresolvedDependencies(_, _) => acc,
                    _ => Self::Success(package),
                }
            }
        })
    }
}

// // let callback_invocation = CallbackInvocation {
// //     components: vec![
// //         "custom_validation_package".into(),
// //         // @todo zome_id is a u8, is this really an ergonomic way for us to interact with
// //         // entry types at the happ code level?
// //         format!("{}", app_entry_type.zome_id()),
// //     ],
// //     zome_name: zome_name.clone(),
// //     payload: HostInput::new(app_entry_type.try_into()?),
// // };
// // let mut callback_outputs: Vec<Option<GuestOutput>> =
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
// match self.call_iterator(validation_package_invocation.into()).nth(0) {
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
