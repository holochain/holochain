use crate::core::ribosome::AllowSideEffects;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;
use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
use crate::fixt::DnaDefFixturator;
use fixt::prelude::*;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::DnaDef;
use holochain_zome_types::init::InitCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;

#[derive(Clone)]
pub struct InitInvocation {
    // @todo InitWorkspace?
    workspace: UnsafeInvokeZomeWorkspace,
    dna_def: DnaDef,
}

fixturator!(
    InitInvocation,
    {
        let init_invocation = InitInvocation {
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
            dna_def: DnaDefFixturator::new_indexed(Empty, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        init_invocation
    },
    {
        let init_invocation = InitInvocation {
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new_indexed(
                Unpredictable,
                self.0.index,
            )
            .next()
            .unwrap(),
            dna_def: DnaDefFixturator::new_indexed(Unpredictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        init_invocation
    },
    {
        let init_invocation = InitInvocation {
            workspace: UnsafeInvokeZomeWorkspaceFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            dna_def: DnaDefFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
        };
        self.0.index = self.0.index + 1;
        init_invocation
    }
);

impl Invocation for InitInvocation {
    fn allow_side_effects(&self) -> AllowSideEffects {
        AllowSideEffects::Yes
    }
    fn zome_names(&self) -> Vec<ZomeName> {
        self.dna_def
            .zomes
            .iter()
            .map(|(zome_name, _)| zome_name.clone())
            .collect()
    }
    fn fn_components(&self) -> FnComponents {
        vec!["init".into()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new(().try_into()?))
    }
    fn workspace(&self) -> UnsafeInvokeZomeWorkspace {
        self.workspace.clone()
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
    /// Vec<EntryHash> is the list of all missing dependency addresses
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
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
mod test {

    use super::InitInvocationFixturator;
    use super::InitResult;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::curve::Zomes;
    use crate::fixt::WasmRibosomeFixturator;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_unimplemented() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(init_invocation).unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_implemented_pass() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(init_invocation).unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_implemented_fail() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::InitFail]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(init_invocation).unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_init_multi_implemented_fail() {
        let ribosome =
            WasmRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass, TestWasm::InitFail]))
                .next()
                .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna.clone();

        let result = ribosome.run_init(init_invocation).unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }
}
