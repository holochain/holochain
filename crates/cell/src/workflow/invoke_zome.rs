use crate::{
    cell::{autonomic::AutonomicCue, error::CellResult},
    conductor_api::ConductorCellApiT,
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    ribosome::{Ribosome, RibosomeT},
    txn::source_chain,
};
use sx_types::shims::*;

pub async fn invoke_zome<'env, Ribo: RibosomeT, Api: ConductorCellApiT>(
    invocation: ZomeInvocation,
    ribosome: Ribo,
    conductor_api: Api,
) -> CellResult<ZomeInvocationResult> {
    unimplemented!();
    // let mut bundle = source_chain.bundle()?;
    let mut bundle = SourceChainCommitBundle::new();
    let result = ribosome.call_zome_function(&mut bundle, invocation)?;
    // let snapshot = source_chain.try_commit(bundle)?;
    Ok(result)
}

#[cfg(test_TODO_FIX)]
pub mod tests {
    use super::*;
    use crate::{
        agent::{source_chain::tests::test_initialized_chain, SourceChainCommitBundle},
        conductor_api::MockConductorCellApi,
        ribosome::MockRibosomeT,
        test_utils::fake_cell_id,
    };
    use source_chain::SourceChainPersistence;
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
        ribosome.expect_call_zome_function()
            .times(1)
            .returning(|bundle, _| Ok(ZomeInvocationResult));

        // TODO: make actual assertions on the conductor_api, once more of the
        // actual logic is fleshed out
        let mut conductor_api = MockConductorCellApi::new();

        let result = invoke_zome(invocation, chain, ribosome, conductor_api).await;
        assert!(result.is_ok());
    }

    // TODO: can try making a fake (not mock) ribosome that has some hard-coded logic
    // for calling into a ZomeApi, rather than needing to write a test DNA. This will
    // have to wait until the whole Ribosome thing is more fleshed out.
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
    //     ) -> SkunkResult<(ZomeInvocationResult, SourceChainCommitBundle)> {
    //         unimplemented!()
    //     }
    // }
}
