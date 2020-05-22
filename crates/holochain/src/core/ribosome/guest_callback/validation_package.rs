use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::fixt::ZomeNameFixturator;
use fixt::prelude::*;
use holo_hash::EntryContentHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::fixt::AppEntryTypeFixturator;
use holochain_types::header::AppEntryType;
use holochain_zome_types::validate::ValidationPackage;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct ValidationPackageInvocation {
    zome_name: ZomeName,
    app_entry_type: AppEntryType,
}

fixturator!(
    ValidationPackageInvocation,
    {
        let validation_package_invocation = ValidationPackageInvocation {
            zome_name: ZomeNameFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            app_entry_type: AppEntryTypeFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        validation_package_invocation
    },
    {
        let validation_package_invocation = ValidationPackageInvocation {
            zome_name: ZomeNameFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
            app_entry_type: AppEntryTypeFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        validation_package_invocation
    },
    {
        let validation_package_invocation = ValidationPackageInvocation {
            zome_name: ZomeNameFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            app_entry_type: AppEntryTypeFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        validation_package_invocation
    }
);

impl Invocation for ValidationPackageInvocation {
    fn allow_side_effects(&self) -> bool {
        false
    }
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::One(self.zome_name.to_owned())
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

#[derive(Debug, PartialEq)]
pub enum ValidationPackageResult {
    Success(ValidationPackage),
    Fail(String),
    UnresolvedDependencies(Vec<EntryContentHash>),
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
                    ValidationPackageCallbackResult::Fail(fail_string) => Self::Fail(fail_string),
                    ValidationPackageCallbackResult::UnresolvedDependencies(ud) => {
                        match acc {
                            // failure anywhere overrides unresolved deps
                            Self::Fail(_) => acc,
                            // unresolved deps overrides anything other than failure
                            _ => Self::UnresolvedDependencies(
                                ud.into_iter().map(|h| h.into()).collect(),
                            ),
                        }
                    }
                    ValidationPackageCallbackResult::Success(package) => match acc {
                        // fail anywhere overrides success
                        Self::Fail(_) => acc,
                        // unresolved deps anywhere overrides success anywhere
                        Self::UnresolvedDependencies(_) => acc,
                        _ => Self::Success(package),
                    },
                }
            })
    }
}

#[cfg(test)]
mod test {

    use super::ValidationPackageInvocationFixturator;
    use super::ValidationPackageResult;
    use crate::core::ribosome::RibosomeT;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::validate::ValidationPackage;

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_validation_package_unimplemented() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut validation_package_invocation =
            ValidationPackageInvocationFixturator::new(fixt::Empty)
                .next()
                .unwrap();
        validation_package_invocation.zome_name = TestWasm::Foo.into();

        let result = ribosome
            .run_validation_package(workspace, validation_package_invocation)
            .unwrap();
        assert_eq!(result, ValidationPackageResult::NotImplemented,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_validation_package_implemented_success() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidationPackageSuccess]))
            .next()
            .unwrap();
        let mut validation_package_invocation =
            ValidationPackageInvocationFixturator::new(fixt::Empty)
                .next()
                .unwrap();
        validation_package_invocation.zome_name = TestWasm::ValidationPackageSuccess.into();

        let result = ribosome
            .run_validation_package(workspace, validation_package_invocation)
            .unwrap();
        assert_eq!(result, ValidationPackageResult::Success(ValidationPackage),);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_validation_package_implemented_fail() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::ValidationPackageFail]))
            .next()
            .unwrap();
        let mut validation_package_invocation =
            ValidationPackageInvocationFixturator::new(fixt::Empty)
                .next()
                .unwrap();
        validation_package_invocation.zome_name = TestWasm::ValidationPackageFail.into();

        let result = ribosome
            .run_validation_package(workspace, validation_package_invocation)
            .unwrap();
        assert_eq!(result, ValidationPackageResult::Fail("bad package".into()),);
    }
}
