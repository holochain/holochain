use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use crate::fixt::DnaDefFixturator;
use fixt::prelude::*;
use holo_hash::EntryContentHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::{zome::HostFnAccess, DnaDef};
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Debug, Clone)]
pub struct InitInvocation {
    pub dna_def: DnaDef,
}

impl InitInvocation {
    pub fn new(dna_def: DnaDef) -> Self {
        Self { dna_def }
    }
}

fixturator!(
    InitInvocation;
    constructor fn new(DnaDef);
);

impl Invocation for InitInvocation {
    fn allowed_access(&self) -> HostFnAccess {
        HostFnAccess::all()
    }
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::All
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
    /// Vec<EntryContentHash> is the list of all missing dependency addresses
    UnresolvedDependencies(ZomeName, Vec<EntryContentHash>),
}

impl From<Vec<InitCallbackResult>> for InitResult {
    fn from(callback_results: Vec<InitCallbackResult>) -> Self {
        callback_results
            .into_iter()
            .fold(Self::Pass, |acc, x| match x {
                // fail overrides everything
                InitCallbackResult::Fail(zome_name, fail_string) => {
                    Self::Fail(zome_name, fail_string)
                }
                // unresolved deps overrides pass but not fail
                InitCallbackResult::UnresolvedDependencies(zome_name, ud) => match acc {
                    Self::Fail(_, _) => acc,
                    _ => Self::UnresolvedDependencies(
                        zome_name,
                        ud.into_iter().map(|h| h.into()).collect(),
                    ),
                },
                // passing callback allows the acc to carry forward
                InitCallbackResult::Pass => acc,
            })
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod test {

    use super::InitInvocationFixturator;
    use super::InitResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeNameFixturator;
    use holochain_serialized_bytes::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::init::InitCallbackResult;
    use holochain_zome_types::HostInput;
    use rand::prelude::*;

    #[tokio::test(threaded_scheduler)]
    async fn init_callback_result_fold() {
        let mut rng = thread_rng();

        let result_pass = || InitResult::Pass;
        let result_ud = || {
            InitResult::UnresolvedDependencies(
                ZomeNameFixturator::new(fixt::Predictable).next().unwrap(),
                vec![],
            )
        };
        let result_fail = || {
            InitResult::Fail(
                ZomeNameFixturator::new(fixt::Predictable).next().unwrap(),
                "".into(),
            )
        };

        let cb_pass = || InitCallbackResult::Pass;
        let cb_ud = || {
            InitCallbackResult::UnresolvedDependencies(
                ZomeNameFixturator::new(fixt::Predictable).next().unwrap(),
                vec![],
            )
        };
        let cb_fail = || {
            InitCallbackResult::Fail(
                ZomeNameFixturator::new(fixt::Predictable).next().unwrap(),
                "".into(),
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

    #[tokio::test(threaded_scheduler)]
    async fn init_invocation_allow_side_effects() {
        let init_invocation = InitInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        assert!(init_invocation.allow_side_effects());
    }

    #[tokio::test(threaded_scheduler)]
    async fn init_invocation_zomes() {
        let init_invocation = InitInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(ZomesToInvoke::All, init_invocation.zomes(),);
    }

    #[tokio::test(threaded_scheduler)]
    async fn init_invocation_fn_components() {
        let init_invocation = InitInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["init"];
        for fn_component in init_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn init_invocation_host_input() {
        let init_invocation = InitInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let host_input = init_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            HostInput::new(SerializedBytes::try_from(()).unwrap()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_unimplemented() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(workspace, init_invocation).unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_implemented_pass() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(workspace, init_invocation).unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_implemented_fail() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::InitFail]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(workspace, init_invocation).unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_multi_implemented_fail() {
        let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let ribosome =
            WasmRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass, TestWasm::InitFail]))
                .next()
                .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(workspace, init_invocation).unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }
}
