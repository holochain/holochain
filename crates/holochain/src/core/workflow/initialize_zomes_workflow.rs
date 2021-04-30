use super::error::WorkflowResult;
use super::CallZomeWorkspace;
use crate::core::ribosome::guest_callback::init::InitHostAccess;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::RibosomeT;
use derive_more::Constructor;
use holochain_keystore::KeystoreSender;
use holochain_p2p::HolochainP2pCell;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::dna::DnaDef;
use holochain_zome_types::header::builder;
use tracing::*;

#[derive(Constructor, Debug)]
pub struct InitializeZomesWorkflowArgs<Ribosome: RibosomeT> {
    pub dna_def: DnaDef,
    pub ribosome: Ribosome,
}

pub type InitializeZomesWorkspace = CallZomeWorkspace;

#[instrument(skip(network, keystore, workspace))]
pub async fn initialize_zomes_workflow<'env, Ribosome: RibosomeT>(
    workspace: HostFnWorkspace,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult> {
    let result =
        initialize_zomes_workflow_inner(workspace.clone(), network, keystore, args).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---
    workspace.flush()?;
    Ok(result)
}

async fn initialize_zomes_workflow_inner<'env, Ribosome: RibosomeT>(
    workspace: HostFnWorkspace,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult> {
    let InitializeZomesWorkflowArgs { dna_def, ribosome } = args;
    // Call the init callback
    let result = {
        let host_access = InitHostAccess::new(workspace.clone(), keystore, network);
        let invocation = InitInvocation { dna_def };
        ribosome.run_init(host_access, invocation)?
    };

    // Insert the init marker
    workspace
        .source_chain()
        .put(builder::InitZomesComplete {}, None)
        .await?;

    // TODO: Validate scratch items

    Ok(result)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::workflow::fake_genesis;
    use crate::fixt::DnaDefFixturator;
    use crate::fixt::KeystoreSenderFixturator;
    use ::fixt::prelude::*;
    use fixt::Unpredictable;
    use holochain_p2p::HolochainP2pCellFixturator;
    use holochain_state::prelude::test_cell_env;
    use holochain_zome_types::Header;
    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    async fn adds_init_marker() {
        let test_env = test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();
        let mut ribosome = MockRibosomeT::new();

        // Setup the ribosome mock
        ribosome
            .expect_run_init()
            .returning(move |_workspace, _invocation| Ok(InitResult::Pass));

        // Genesis
        fake_genesis(&mut workspace.source_chain).await.unwrap();

        let dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();

        let args = InitializeZomesWorkflowArgs { ribosome, dna_def };
        let keystore = fixt!(KeystoreSender);
        let network = fixt!(HolochainP2pCell);
        let workspace_lock = CallZomeWorkspaceLock::new(workspace);
        initialize_zomes_workflow_inner(workspace_lock.clone(), network, keystore, args)
            .await
            .unwrap();

        // Check init is added to the workspace
        assert_matches!(
            workspace_lock
                .read()
                .await
                .source_chain
                .get_at_index(3)
                .unwrap()
                .unwrap()
                .header(),
            Header::InitZomesComplete(_)
        );
    }
}
