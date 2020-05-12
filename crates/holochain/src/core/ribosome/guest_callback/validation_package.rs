use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::header::AppEntryType;
use holochain_zome_types::validate::ValidationPackage;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct ValidationPackageInvocation {
    // @todo ValidationPackageWorkspace?
    workspace: UnsafeInvokeZomeWorkspace,
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
        vec![
            "validation_package".into(),
            format!("{}", self.app_entry_type.zome_id()),
        ]
        .into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new((&self.app_entry_type).try_into()?))
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace.clone()
    }
}

impl TryFrom<ValidationPackageInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(
        validation_package_invocation: ValidationPackageInvocation,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(
            (&validation_package_invocation.app_entry_type).try_into()?,
        ))
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
        callback_results
            .into_iter()
            .fold(Self::NotImplemented, |acc, x| {
                match x {
                    ValidationPackageCallbackResult::Fail(zome_name, fail_string) => {
                        Self::Fail(zome_name, fail_string)
                    }
                    ValidationPackageCallbackResult::UnresolvedDependencies(zome_name, ud) => {
                        match acc {
                            // failure anywhere overrides unresolved deps
                            Self::Fail(_, _) => acc,
                            // unresolved deps overrides anything other than failure
                            _ => Self::UnresolvedDependencies(
                                zome_name,
                                ud.into_iter().map(|h| h.into()).collect(),
                            ),
                        }
                    }
                    ValidationPackageCallbackResult::Success(package) => match acc {
                        // fail anywhere overrides success
                        Self::Fail(_, _) => acc,
                        // unresolved deps anywhere overrides success anywhere
                        Self::UnresolvedDependencies(_, _) => acc,
                        _ => Self::Success(package),
                    },
                }
            })
    }
}
