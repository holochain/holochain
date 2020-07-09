use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::fixt::ZomeNameFixturator;
use fixt::prelude::*;
use holo_hash::EntryContentHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::fixt::AppEntryTypeFixturator;
use holochain_types::{dna::zome::HostFnAccess, header::AppEntryType};
use holochain_zome_types::validate::ValidationPackage;
use holochain_zome_types::validate::ValidationPackageCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct ValidationPackageInvocation {
    zome_name: ZomeName,
    app_entry_type: AppEntryType,
}

impl ValidationPackageInvocation {
    pub fn new(zome_name: ZomeName, app_entry_type: AppEntryType) -> Self {
        Self {
            zome_name,
            app_entry_type,
        }
    }
}

fixturator!(
    ValidationPackageInvocation;
    constructor fn new(ZomeName, AppEntryType);
);

impl Invocation for ValidationPackageInvocation {
    fn allowed_access(&self) -> HostFnAccess {
        HostFnAccess::none()
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
#[cfg(feature = "slow_tests")]
mod test {

    use super::ValidationPackageInvocationFixturator;
    use super::ValidationPackageResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use holochain_serialized_bytes::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::validate::ValidationPackage;
    use holochain_zome_types::validate::ValidationPackageCallbackResult;
    use holochain_zome_types::HostInput;
    use rand::prelude::*;

    #[tokio::test(threaded_scheduler)]
    async fn validate_package_callback_result_fold() {
        let mut rng = thread_rng();

        let result_success = || ValidationPackageResult::Success(ValidationPackage);
        let result_ud = || ValidationPackageResult::UnresolvedDependencies(vec![]);
        let result_fail = || ValidationPackageResult::Fail("".into());
        let result_not_implemented = || ValidationPackageResult::NotImplemented;

        let cb_success = || ValidationPackageCallbackResult::Success(ValidationPackage);
        let cb_ud = || ValidationPackageCallbackResult::UnresolvedDependencies(vec![]);
        let cb_fail = || ValidationPackageCallbackResult::Fail("".into());

        for (mut results, expected) in vec![
            (vec![], result_not_implemented()),
            (vec![cb_success()], result_success()),
            (vec![cb_fail()], result_fail()),
            (vec![cb_ud()], result_ud()),
            (vec![cb_fail(), cb_success()], result_fail()),
            (vec![cb_fail(), cb_ud()], result_fail()),
            (vec![cb_success(), cb_ud()], result_ud()),
            (vec![cb_success(), cb_ud(), cb_fail()], result_fail()),
        ] {
            // order of the results should not change the final result
            results.shuffle(&mut rng);

            // number of times a callback result appears should not change the final result
            let number_of_extras = rng.gen_range(0, 5);
            for _ in 0..number_of_extras {
                let maybe_extra = results.choose(&mut rng).cloned();
                match maybe_extra {
                    Some(extra) => results.push(extra),
                    _ => {}
                };
            }

            assert_eq!(expected, results.into(),);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn validation_package_invocation_allow_side_effects() {
        let validation_package_invocation =
            ValidationPackageInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();
        assert!(!validation_package_invocation.allow_side_effects());
    }

    #[tokio::test(threaded_scheduler)]
    async fn validation_package_invocation_zomes() {
        let validation_package_invocation =
            ValidationPackageInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();
        let zome_name = validation_package_invocation.zome_name.clone();
        assert_eq!(
            ZomesToInvoke::One(zome_name),
            validation_package_invocation.zomes(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn validation_package_invocation_fn_components() {
        let validation_package_invocation =
            ValidationPackageInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();

        let mut expected = vec![
            "validation_package".to_string(),
            format!(
                "validation_package_{}",
                validation_package_invocation.app_entry_type.zome_id()
            ),
        ];
        for fn_component in validation_package_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap(),);
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn validation_package_invocation_host_input() {
        let validation_package_invocation =
            ValidationPackageInvocationFixturator::new(fixt::Unpredictable)
                .next()
                .unwrap();

        let host_input = validation_package_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            HostInput::new(
                SerializedBytes::try_from(&validation_package_invocation.app_entry_type).unwrap()
            ),
        );
    }

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
