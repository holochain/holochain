use super::{WorkflowEffects, WorkflowResult};
use crate::core::{ribosome::RibosomeT, state::workspace::InvokeZomeWorkspace};
use sx_types::nucleus::ZomeInvocation;

pub async fn invoke_zome<'env, Ribo: RibosomeT>(
    workspace: InvokeZomeWorkspace<'_>,
    _ribosome: Ribo,
    _invocation: ZomeInvocation,
) -> WorkflowResult<InvokeZomeWorkspace<'_>> {
    Ok(WorkflowEffects {
        workspace,
        triggers: Default::default(),
        signals: Default::default(),
        callbacks: Default::default(),
    })
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
    use sx_types::{entry::Entry, error::SkunkResult};
    use tempdir::TempDir;

    #[tokio::test]
    async fn can_invoke_zome_with_mock() {
        let cell_id = fake_cell_id("mario");
        let tmpdir = TempDir::new("skunkworx").unwrap();
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
