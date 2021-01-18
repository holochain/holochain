use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostAccess;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::workflow::CallZomeWorkspaceLock;
use derive_more::Constructor;
use holo_hash::AnyDhtHash;
use holochain_p2p::HolochainP2pCell;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;

#[derive(Clone)]
pub struct ValidationPackageInvocation {
    zome: Zome,
    app_entry_type: AppEntryType,
}

impl ValidationPackageInvocation {
    pub fn new(zome: Zome, app_entry_type: AppEntryType) -> Self {
        Self {
            zome,
            app_entry_type,
        }
    }
}

#[derive(Clone, Constructor)]
pub struct ValidationPackageHostAccess {
    pub workspace: CallZomeWorkspaceLock,
    pub network: HolochainP2pCell,
}

impl From<ValidationPackageHostAccess> for HostAccess {
    fn from(validation_package_host_access: ValidationPackageHostAccess) -> Self {
        Self::ValidationPackage(validation_package_host_access)
    }
}

impl From<&ValidationPackageHostAccess> for HostFnAccess {
    fn from(_: &ValidationPackageHostAccess) -> Self {
        let mut access = Self::none();
        access.read_workspace = Permission::Allow;
        access.agent_info = Permission::Allow;
        access
    }
}

impl Invocation for ValidationPackageInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::One(self.zome.to_owned())
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
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(self.app_entry_type)
    }
}

impl TryFrom<ValidationPackageInvocation> for ExternIO {
    type Error = SerializedBytesError;
    fn try_from(
        validation_package_invocation: ValidationPackageInvocation,
    ) -> Result<Self, Self::Error> {
        Self::encode(&validation_package_invocation.app_entry_type)
    }
}

#[derive(Debug, PartialEq)]
pub enum ValidationPackageResult {
    Success(ValidationPackage),
    Fail(String),
    UnresolvedDependencies(Vec<AnyDhtHash>),
    NotImplemented,
}

impl From<Vec<(ZomeName, ValidationPackageCallbackResult)>> for ValidationPackageResult {
    fn from(a: Vec<(ZomeName, ValidationPackageCallbackResult)>) -> Self {
        a.into_iter().map(|(_, v)| v).collect::<Vec<_>>().into()
    }
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
                            _ => Self::UnresolvedDependencies(ud.into_iter().collect()),
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
    use super::ValidationPackageResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::ValidationPackageHostAccessFixturator;
    use crate::fixt::ValidationPackageInvocationFixturator;
    use holochain_types::dna::zome::HostFnAccess;
    use holochain_zome_types::validate::ValidationPackage;
    use holochain_zome_types::validate::ValidationPackageCallbackResult;
    use holochain_zome_types::ExternIO;
    use rand::prelude::*;

    #[tokio::test(threaded_scheduler)]
    async fn validate_package_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_success = || ValidationPackageResult::Success(ValidationPackage(vec![]));
        let result_ud = || ValidationPackageResult::UnresolvedDependencies(vec![]);
        let result_fail = || ValidationPackageResult::Fail("".into());
        let result_not_implemented = || ValidationPackageResult::NotImplemented;

        let cb_success = || ValidationPackageCallbackResult::Success(ValidationPackage(vec![]));
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
        use holochain_types::dna::zome::Permission::*;
        let validation_package_host_access =
            ValidationPackageHostAccessFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();
        assert_eq!(
            HostFnAccess::from(&validation_package_host_access),
            HostFnAccess {
                agent_info: Allow,
                read_workspace: Allow,
                write_workspace: Deny,
                write_network: Deny,
                dna_bindings: Deny,
                non_determinism: Deny,
                keystore: Deny,
            }
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn validation_package_invocation_zomes() {
        let validation_package_invocation =
            ValidationPackageInvocationFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();
        let zome = validation_package_invocation.zome.clone();
        assert_eq!(
            ZomesToInvoke::One(zome),
            validation_package_invocation.zomes(),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn validation_package_invocation_fn_components() {
        let validation_package_invocation =
            ValidationPackageInvocationFixturator::new(::fixt::Unpredictable)
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
            ValidationPackageInvocationFixturator::new(::fixt::Unpredictable)
                .next()
                .unwrap();

        let host_input = validation_package_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            ExternIO::encode(&validation_package_invocation.app_entry_type).unwrap(),
        );
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::ValidationPackageResult;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::ValidationPackageHostAccessFixturator;
    use crate::fixt::ValidationPackageInvocationFixturator;
    use hdk3::prelude::AppEntryType;
    use hdk3::prelude::EntryVisibility;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::validate::ValidationPackage;

    #[tokio::test(threaded_scheduler)]
    async fn test_validation_package_unimplemented() {
        let host_access = ValidationPackageHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut validation_package_invocation =
            ValidationPackageInvocationFixturator::new(::fixt::Empty)
                .next()
                .unwrap();
        validation_package_invocation.zome = TestWasm::Foo.into();

        let result = ribosome
            .run_validation_package(host_access, validation_package_invocation)
            .unwrap();
        assert_eq!(result, ValidationPackageResult::NotImplemented,);
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validation_package_implemented_success() {
        let host_access = ValidationPackageHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidationPackageSuccess]))
            .next()
            .unwrap();
        let mut validation_package_invocation =
            ValidationPackageInvocationFixturator::new(::fixt::Empty)
                .next()
                .unwrap();
        validation_package_invocation.zome = TestWasm::ValidationPackageSuccess.into();
        validation_package_invocation.app_entry_type =
            AppEntryType::new(3.into(), 0.into(), EntryVisibility::Public);

        let result = ribosome
            .run_validation_package(host_access, validation_package_invocation)
            .unwrap();
        assert_eq!(
            result,
            ValidationPackageResult::Success(ValidationPackage(vec![])),
        );
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_validation_package_implemented_fail() {
        let host_access = ValidationPackageHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::ValidationPackageFail]))
            .next()
            .unwrap();
        let mut validation_package_invocation =
            ValidationPackageInvocationFixturator::new(::fixt::Empty)
                .next()
                .unwrap();
        validation_package_invocation.zome = TestWasm::ValidationPackageFail.into();

        let result = ribosome
            .run_validation_package(host_access, validation_package_invocation)
            .unwrap();
        assert_eq!(result, ValidationPackageResult::Fail("bad package".into()),);
    }
}
