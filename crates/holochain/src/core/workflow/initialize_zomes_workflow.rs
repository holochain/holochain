use super::error::WorkflowResult;
use super::CallZomeWorkspace;
use super::CallZomeWorkspaceLock;
use crate::core::queue_consumer::OneshotWriter;
use crate::core::ribosome::guest_callback::init::InitHostAccess;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::RibosomeT;
use derive_more::Constructor;
use holochain_keystore::KeystoreSender;
use holochain_p2p::HolochainP2pCell;
use holochain_state::workspace::Workspace;
use holochain_types::dna::DnaDef;
use holochain_zome_types::header::builder;
use tracing::*;

#[derive(Constructor, Debug)]
pub struct InitializeZomesWorkflowArgs<Ribosome: RibosomeT> {
    pub dna_def: DnaDef,
    pub ribosome: Ribosome,
}

pub type InitializeZomesWorkspace = CallZomeWorkspace;

#[instrument(skip(network, keystore, workspace, writer))]
pub async fn initialize_zomes_workflow<'env, Ribosome: RibosomeT>(
    workspace: InitializeZomesWorkspace,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    writer: OneshotWriter,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult> {
    let workspace_lock = CallZomeWorkspaceLock::new(workspace);
    let result =
        initialize_zomes_workflow_inner(workspace_lock.clone(), network, keystore, args).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---
    {
        let mut guard = workspace_lock.write().await;
        let workspace: &mut CallZomeWorkspace = &mut guard;
        // commit the workspace
        writer.with_writer(|writer| Ok(workspace.flush_to_txn_ref(writer)?))?;
    }
    Ok(result)
}

async fn initialize_zomes_workflow_inner<'env, Ribosome: RibosomeT>(
    workspace: CallZomeWorkspaceLock,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult> {
    let InitializeZomesWorkflowArgs { dna_def, ribosome } = args;
    // Call the init callback
    let result = {
        // TODO: We need a better solution then re-using the CallZomeWorkspace (i.e. ghost actor)
        let host_access = InitHostAccess::new(workspace.clone(), keystore, network);
        let invocation = InitInvocation { dna_def };
        ribosome.run_init(host_access, invocation)?
    };

    // Insert the init marker
    workspace
        .write()
        .await
        .source_chain
        .put(builder::InitZomesComplete {}, None)
        .await?;

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
    use holochain_lmdb::test_utils::test_cell_env;
    use holochain_p2p::HolochainP2pCellFixturator;
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
