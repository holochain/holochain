use super::error::WorkflowResult;
use crate::conductor::api::CellConductorApiT;
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
pub struct InitializeZomesWorkflowArgs<Ribosome, C>
where
    Ribosome: RibosomeT + Send + 'static,
    C: CellConductorApiT,
{
    pub dna_def: DnaDef,
    pub ribosome: Ribosome,
    pub conductor_api: C,
}

#[instrument(skip(network, keystore, workspace, args))]
pub async fn initialize_zomes_workflow<Ribosome, C>(
    workspace: HostFnWorkspace,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    args: InitializeZomesWorkflowArgs<Ribosome, C>,
) -> WorkflowResult<InitResult>
where
    Ribosome: RibosomeT + Send + 'static,
    C: CellConductorApiT,
{
    let result =
        initialize_zomes_workflow_inner(workspace.clone(), network, keystore, args).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---
    workspace.flush()?;
    Ok(result)
}

async fn initialize_zomes_workflow_inner<Ribosome, C>(
    workspace: HostFnWorkspace,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    args: InitializeZomesWorkflowArgs<Ribosome, C>,
) -> WorkflowResult<InitResult>
where
    Ribosome: RibosomeT + Send + 'static,
    C: CellConductorApiT,
{
    let InitializeZomesWorkflowArgs {
        dna_def,
        ribosome,
        conductor_api,
    } = args;
    // Call the init callback
    let result = {
        let host_access = InitHostAccess::new(workspace.clone(), keystore, network.clone());
        let invocation = InitInvocation { dna_def };
        ribosome.run_init(host_access, invocation)?
    };

    // Insert the init marker
    workspace
        .source_chain()
        .put(builder::InitZomesComplete {}, None)
        .await?;

    // TODO: Validate scratch items
    super::inline_validation(workspace, network, conductor_api, None, ribosome).await?;

    Ok(result)
}

#[cfg(test)]
pub mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::conductor::api::CellConductorApi;
    use crate::conductor::handle::MockConductorHandleT;
    use crate::core::ribosome::MockRibosomeT;
    use crate::fixt::DnaDefFixturator;
    use crate::fixt::KeystoreSenderFixturator;
    use crate::fixt::*;
    use crate::test_utils::fake_genesis;
    use ::fixt::prelude::*;
    use fixt::Unpredictable;
    use holo_hash::DnaHash;
    use holochain_p2p::HolochainP2pCellFixturator;
    use holochain_state::prelude::test_cache_env;
    use holochain_state::prelude::test_cell_env;
    use holochain_types::prelude::DnaDefHashed;
    use holochain_zome_types::fake_agent_pubkey_1;
    use holochain_zome_types::CellId;
    use holochain_zome_types::Header;
    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    async fn adds_init_marker() {
        let test_env = test_cell_env();
        let test_cache = test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();

        // Genesis
        fake_genesis(env.clone()).await.unwrap();

        let workspace =
            HostFnWorkspace::new(env.clone(), test_cache.env(), author.clone()).unwrap();
        let mut ribosome = MockRibosomeT::new();
        let dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();
        let dna_hash = DnaHash::with_data_sync(&dna_def);
        let dna_def_hashed = DnaDefHashed::from_content_sync(dna_def.clone());
        // Setup the ribosome mock
        ribosome
            .expect_run_init()
            .returning(move |_workspace, _invocation| Ok(InitResult::Pass));
        ribosome.expect_dna_def().return_const(dna_def_hashed);

        let cell_id = CellId::new(dna_hash, fixt!(AgentPubKey));
        let conductor_api = Arc::new(MockConductorHandleT::new());
        let conductor_api = CellConductorApi::new(conductor_api, cell_id);
        let args = InitializeZomesWorkflowArgs {
            ribosome,
            dna_def,
            conductor_api,
        };
        let keystore = fixt!(KeystoreSender);
        let network = fixt!(HolochainP2pCell);
        initialize_zomes_workflow_inner(workspace.clone(), network, keystore, args)
            .await
            .unwrap();

        // Check init is added to the workspace
        let scratch = workspace.source_chain().snapshot().unwrap();
        assert_matches!(
            scratch.headers().next().unwrap().header(),
            Header::InitZomesComplete(_)
        );
    }
}
