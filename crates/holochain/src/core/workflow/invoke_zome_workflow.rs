use super::Workspace;
use super::{error::WorkflowResult, Workflow, WorkflowEffects};
use crate::core::ribosome::RibosomeT;
use crate::core::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use futures::future::FutureExt;
use holochain_state::prelude::*;
use holochain_types::{nucleus::ZomeInvocation, prelude::Todo};
use must_future::MustBoxFuture;

pub type ZomeInvocationResult = Todo;

pub struct InvokeZomeWorkflow<Ribosome: RibosomeT> {
    ribosome: Ribosome,
    invocation: ZomeInvocation,
}

impl<'env, Ribosome: RibosomeT + Send + Sync> Workflow<'env> for InvokeZomeWorkflow<Ribosome> {
    type Output = ZomeInvocationResult;
    type Workspace = InvokeZomeWorkspace<'env>;
    type Triggers = ();

    // TODO: remove when implemented
    #[allow(unreachable_code, unused_variables)]
    fn workflow(
        self,
        // environment: &'env EnvironmentRo,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>> {
        async {
            // let env = environment.guard().await;
            // let mut workspace = InvokeZomeWorkspace::new(&env.reader()?, &env)?;
            let fx = WorkflowEffects::new(workspace, Default::default(), Default::default(), ());
            let result = todo!("this will be the actual zome function return value");
            Ok((result, fx))
        }
        .boxed()
        .into()
    }
}

pub struct InvokeZomeWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(_reader: &Reader<'env>, _dbs: &impl GetDb) -> WorkspaceResult<Self> {
        unimplemented!()
    }
}
impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}

#[cfg(test_TODO_FIX)]
pub mod tests {
    use super::*;
    use crate::{
        agent::{source_chain::tests::test_initialized_chain, SourceChainCommitBundle},
        conductor_api::MockCellConductorApi,
        ribosome::MockRibosomeT,
        test_utils::fake_cell_id,
    };
    use holochain_types::{entry::Entry, error::SkunkResult};
    use tempdir::TempDir;

    #[tokio::test]
    async fn can_invoke_zome_with_mock() {
        let cell_id = fake_cell_id("mario");
        let tmpdir = TempDir::new("holochain_2020").unwrap();
        let persistence = SourceChainPersistence::test(tmpdir.path());
        let chain = test_initialized_chain("mario", &persistence);
        let invocation = ZomeInvocation {
            cell_id: cell_id.clone(),
            zome_name: "zome".into(),
            fn_name: "fn".into(),
            as_at: "KwyXHisn".into(),
            args: "args".into(),
            provenance: cell_id.agent_id().to_owned(),
            cap: CapabilityRequest,
        };

        let mut ribosome = MockRibosomeT::new();
        ribosome
            .expect_call_zome_function()
            .times(1)
            .returning(|bundle, _| Ok(ZomeInvocationResponse));

        // TODO: make actual assertions on the conductor_api, once more of the
        // actual logic is fleshed out
        let mut conductor_api = MockCellConductorApi::new();

        let result = invoke_zome(invocation, chain, ribosome, conductor_api).await;
        assert!(result.is_ok());
    }

    // TODO: can try making a fake (not mock) ribosome that has some hard-coded logic
    // for calling into a ZomeApi, rather than needing to write a test DNA. This will
    // have to wait until the whole WasmRibosome thing is more fleshed out.
    // struct FakeRibosome;

    // impl RibosomeT for FakeRibosome {
    //     fn run_validation(self, cursor: &source_chain::Cursor, entry: Entry) -> ValidationResult {
    //         unimplemented!()
    //     }

    //     /// Runs the specified zome fn. Returns the cursor used by HDK,
    //     /// so that it can be passed on to source chain manager for transactional writes
    //     fn call_zome_function(
    //         self,
    //         bundle: SourceChainCommitBundle,
    //         invocation: ZomeInvocation,
    //     ) -> SkunkResult<(ZomeInvocationResponse, SourceChainCommitBundle)> {
    //         unimplemented!()
    //     }
    // }
}
