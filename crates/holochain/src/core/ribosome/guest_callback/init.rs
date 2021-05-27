use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holo_hash::AnyDhtHash;
use holochain_keystore::KeystoreSender;
use holochain_p2p::HolochainP2pCell;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::prelude::*;

#[derive(Debug, Clone)]
pub struct InitInvocation {
    pub dna_def: DnaDef,
}

impl InitInvocation {
    pub fn new(dna_def: DnaDef) -> Self {
        Self { dna_def }
    }
}

#[derive(Clone, Constructor)]
pub struct InitHostAccess {
    pub workspace: HostFnWorkspace,
    pub keystore: KeystoreSender,
    pub network: HolochainP2pCell,
}

impl From<InitHostAccess> for HostContext {
    fn from(init_host_access: InitHostAccess) -> Self {
        Self::Init(init_host_access)
    }
}

impl From<&InitHostAccess> for HostFnAccess {
    fn from(_: &InitHostAccess) -> Self {
        Self::all()
    }
}

impl Invocation for InitInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::All
    }
    fn fn_components(&self) -> FnComponents {
        vec!["init".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(())
    }
}

impl TryFrom<InitInvocation> for ExternIO {
    type Error = SerializedBytesError;
    fn try_from(_: InitInvocation) -> Result<Self, Self::Error> {
        Self::encode(())
    }
}

/// the aggregate result of _all_ init callbacks
#[derive(PartialEq, Debug)]
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
    UnresolvedDependencies(ZomeName, Vec<AnyDhtHash>),
}

impl From<Vec<(ZomeName, InitCallbackResult)>> for InitResult {
    fn from(callback_results: Vec<(ZomeName, InitCallbackResult)>) -> Self {
        callback_results
            .into_iter()
            .fold(Self::Pass, |acc, (zome_name, x)| match x {
                // fail overrides everything
                InitCallbackResult::Fail(fail_string) => Self::Fail(zome_name, fail_string),
                // unresolved deps overrides pass but not fail
                InitCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Fail(_, _) => acc,
                    _ => Self::UnresolvedDependencies(zome_name, ud.into_iter().collect()),
                },
                // passing callback allows the acc to carry forward
                InitCallbackResult::Pass => acc,
            })
    }
}

#[cfg(test)]
mod test {
    use super::InitResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::InitHostAccessFixturator;
    use crate::fixt::InitInvocationFixturator;
    use crate::fixt::ZomeNameFixturator;
    use ::fixt::prelude::*;
    use holochain_types::prelude::*;
    use holochain_zome_types::init::InitCallbackResult;
    use holochain_zome_types::ExternIO;

    #[test]
    fn init_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_pass = || InitResult::Pass;
        let result_ud = || {
            InitResult::UnresolvedDependencies(
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                vec![],
            )
        };
        let result_fail = || {
            InitResult::Fail(
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                "".into(),
            )
        };

        let cb_pass = || {
            (
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                InitCallbackResult::Pass,
            )
        };
        let cb_ud = || {
            (
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                InitCallbackResult::UnresolvedDependencies(vec![]),
            )
        };
        let cb_fail = || {
            (
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                InitCallbackResult::Fail("".into()),
            )
        };

        for (mut results, expected) in vec![
            (vec![], result_pass()),
            (vec![cb_pass()], result_pass()),
            (vec![cb_fail()], result_fail()),
            (vec![cb_ud()], result_ud()),
            (vec![cb_fail(), cb_pass()], result_fail()),
            (vec![cb_fail(), cb_ud()], result_fail()),
            (vec![cb_pass(), cb_ud()], result_ud()),
            (vec![cb_pass(), cb_fail(), cb_ud()], result_fail()),
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

    #[tokio::test(flavor = "multi_thread")]
    async fn init_access() {
        let init_host_access = InitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(HostFnAccess::from(&init_host_access), HostFnAccess::all(),);
    }

    #[test]
    fn init_invocation_zomes() {
        let init_invocation = InitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(ZomesToInvoke::All, init_invocation.zomes(),);
    }

    #[test]
    fn init_invocation_fn_components() {
        let init_invocation = InitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["init"];
        for fn_component in init_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[test]
    fn init_invocation_host_input() {
        let init_invocation = InitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let host_input = init_invocation.clone().host_input().unwrap();

        assert_eq!(host_input, ExternIO::encode(()).unwrap(),);
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::InitResult;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::InitHostAccessFixturator;
    use crate::fixt::InitInvocationFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use ::fixt::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_unimplemented() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Crud]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome.run_init(host_access, init_invocation).unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_implemented_pass() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome.run_init(host_access, init_invocation).unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_implemented_fail() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::InitFail]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome.run_init(host_access, init_invocation).unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_multi_implemented_fail() {
        let ribosome =
            RealRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass, TestWasm::InitFail]))
                .next()
                .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome.run_init(host_access, init_invocation).unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }
}
