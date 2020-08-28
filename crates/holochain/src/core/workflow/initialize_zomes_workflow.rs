use super::{
    error::WorkflowResult, unsafe_call_zome_workspace::UnsafeCallZomeWorkspace, CallZomeWorkspace,
};
use crate::core::{
    queue_consumer::OneshotWriter,
    ribosome::{
        guest_callback::init::{InitHostAccess, InitInvocation, InitResult},
        RibosomeT,
    },
    state::workspace::{Workspace, WorkspaceResult},
};
use derive_more::Constructor;
use holochain_keystore::KeystoreSender;
use holochain_p2p::HolochainP2pCell;
use holochain_state::buffer::BufferedStore;
use holochain_state::prelude::{EnvironmentRead, GetDb, Writer};
use holochain_types::dna::DnaDef;
use holochain_zome_types::header::builder;
use tracing::*;

#[derive(Constructor, Debug)]
pub struct InitializeZomesWorkflowArgs<Ribosome: RibosomeT> {
    pub dna_def: DnaDef,
    pub ribosome: Ribosome,
}

#[instrument(skip(network, keystore, workspace, writer))]
pub async fn initialize_zomes_workflow<'env, Ribosome: RibosomeT>(
    mut workspace: InitializeZomesWorkspace,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    writer: OneshotWriter,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult> {
    let result = initialize_zomes_workflow_inner(&mut workspace, network, keystore, args).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer
        .with_writer(|writer| Ok(workspace.flush_to_txn(writer)?))
        .await?;

    Ok(result)
}

async fn initialize_zomes_workflow_inner<'env, Ribosome: RibosomeT>(
    workspace: &mut InitializeZomesWorkspace,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult> {
    let InitializeZomesWorkflowArgs { dna_def, ribosome } = args;
    // Call the init callback
    let result = {
        // TODO: We need a better solution then re-using the CallZomeWorkspace (i.e. ghost actor)
        let (_g, raw_workspace) = UnsafeCallZomeWorkspace::from_mut(&mut workspace.0);
        let host_access = InitHostAccess::new(raw_workspace, keystore, network);
        let invocation = InitInvocation { dna_def };
        ribosome.run_init(host_access, invocation)?
    };

    // Insert the init marker
    workspace
        .0
        .source_chain
        .put(builder::InitZomesComplete {}, None)
        .await?;

    Ok(result)
}

// TODO: why pub? -MD
pub struct InitializeZomesWorkspace(pub(crate) CallZomeWorkspace);

impl InitializeZomesWorkspace {
    #[allow(dead_code)]
    /// Constructor
    pub async fn new(env: EnvironmentRead, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self(CallZomeWorkspace::new(env, dbs).await?))
    }
}

impl Workspace for InitializeZomesWorkspace {
    fn flush_to_txn(self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.0.source_chain.into_inner().flush_to_txn(writer)?;
        self.0.meta.flush_to_txn(writer)?;
        self.0.cache_cas.flush_to_txn(writer)?;
        self.0.cache_meta.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::workflow::fake_genesis;
    use crate::fixt::DnaDefFixturator;
    use crate::fixt::KeystoreSenderFixturator;
    use fixt::prelude::*;
    use fixt::Unpredictable;
    use holochain_p2p::HolochainP2pCellFixturator;
    use holochain_state::test_utils::test_cell_env;
    use holochain_zome_types::Header;
    use matches::assert_matches;

    #[tokio::test(threaded_scheduler)]
    async fn adds_init_marker() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let mut workspace = InitializeZomesWorkspace(
            CallZomeWorkspace::new(env.clone().into(), &dbs)
                .await
                .unwrap(),
        );
        let mut ribosome = MockRibosomeT::new();

        // Setup the ribosome mock
        ribosome
            .expect_run_init()
            .returning(move |_workspace, _invocation| Ok(InitResult::Pass));

        // Genesis
        fake_genesis(&mut workspace.0.source_chain).await.unwrap();

        let dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();

        let args = InitializeZomesWorkflowArgs { ribosome, dna_def };
        let keystore = fixt!(KeystoreSender);
        let network = fixt!(HolochainP2pCell);
        initialize_zomes_workflow_inner(&mut workspace, network, keystore, args)
            .await
            .unwrap();

        // Check init is added to the workspace
        assert_matches!(
            *workspace
                .0
                .source_chain
                .get_at_index(3)
                .await
                .unwrap()
                .unwrap()
                .header(),
            Header::InitZomesComplete(_)
        );
    }
}
