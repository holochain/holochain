use super::error::WorkflowResult;
use crate::conductor::api::CellConductorApiT;
use crate::core::ribosome::guest_callback::init::InitHostAccess;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::post_commit::send_post_commit;
use crate::core::ribosome::RibosomeT;
use derive_more::Constructor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pCell;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::prelude::*;
use holochain_zome_types::header::builder;
use tracing::*;

#[derive(Constructor, Debug)]
pub struct InitializeZomesWorkflowArgs<Ribosome, C>
where
    Ribosome: RibosomeT + Send + 'static,
    C: CellConductorApiT + Clone,
{
    pub dna_def: DnaDef,
    pub ribosome: Ribosome,
    pub conductor_api: C,
}

#[instrument(skip(network, keystore, workspace, args))]
pub async fn initialize_zomes_workflow<Ribosome, C>(
    workspace: HostFnWorkspace,
    network: HolochainP2pCell,
    keystore: MetaLairClient,
    args: InitializeZomesWorkflowArgs<Ribosome, C>,
) -> WorkflowResult<InitResult>
where
    Ribosome: RibosomeT + Send + 'static,
    C: CellConductorApiT,
{
    let conductor_api = args.conductor_api.clone();
    let result =
        initialize_zomes_workflow_inner(workspace.clone(), network.clone(), keystore.clone(), args)
            .await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // only commit if the result was successful
    if result == InitResult::Pass {
        let flushed_headers = workspace.clone().flush(&network).await?;
        send_post_commit(conductor_api, workspace, network, keystore, flushed_headers).await?;
    }
    Ok(result)
}

    network: HolochainP2pDna,
    keystore: MetaLairClient,
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
    // FIXME: For some reason if we don't spawn here
    // this future never gets polled again.
    let ws = workspace.clone();
    tokio::task::spawn(async move {
        ws.source_chain()
            .put(
                None,
                builder::InitZomesComplete {},
                None,
                ChainTopOrdering::Strict,
            )
            .await
    })
    .await??;

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
    use crate::fixt::MetaLairClientFixturator;
    use crate::sweettest::*;
    use crate::test_utils::fake_genesis;
    use ::fixt::prelude::*;
    use fixt::Unpredictable;
    use holochain_p2p::HolochainP2pDnaFixturator;
    use holochain_state::prelude::test_cache_env;
    use holochain_state::prelude::test_cell_env;
    use holochain_state::prelude::SourceChain;
    use holochain_types::prelude::DnaDefHashed;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::fake_agent_pubkey_1;
    use holochain_zome_types::CellId;
    use holochain_zome_types::Header;
    use matches::assert_matches;

    async fn get_chain(cell: &SweetCell) -> SourceChain {
        SourceChain::new(cell.env().clone().into(), cell.agent_pubkey().clone())
            .await
            .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn adds_init_marker() {
        let test_env = test_cell_env();
        let test_cache = test_cache_env();
        let env = test_env.env();
        let author = fake_agent_pubkey_1();

        // Genesis
        fake_genesis(env.clone()).await.unwrap();

        let workspace = HostFnWorkspace::new(env.clone(), test_cache.env(), author.clone())
            .await
            .unwrap();
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
        let keystore = fixt!(MetaLairClient);
        let network = fixt!(HolochainP2pDna);

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

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_during_init() {
        // SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create, TestWasm::InitFail])
        let (dna, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
            .await
            .unwrap();
        let mut conductor = SweetConductor::from_standard_config().await;
        let app = conductor.setup_app("app", &[dna]).await.unwrap();
        let (cell,) = app.into_tuple();
        let zome = cell.zome("create_entry");

        assert_eq!(get_chain(&cell).await.len().unwrap(), 3);
        assert_eq!(
            get_chain(&cell)
                .await
                .query(Default::default())
                .await
                .unwrap()
                .len(),
            3
        );

        let _: HeaderHash = conductor.call(&zome, "create_entry", ()).await;

        let source_chain = get_chain(&cell).await;
        // - Ensure that the InitZomesComplete element got committed after the
        //   element committed during init()
        assert_matches!(
            source_chain.query(Default::default()).await.unwrap()[4].header(),
            Header::InitZomesComplete(_)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_during_init_one_zome_passes_one_fails() {
        let (dna, _) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create, TestWasm::InitFail])
                .await
                .unwrap();
        let mut conductor = SweetConductor::from_standard_config().await;
        let app = conductor.setup_app("app", &[dna]).await.unwrap();
        let (cell,) = app.into_tuple();
        let zome = cell.zome("create_entry");

        assert_eq!(get_chain(&cell).await.len().unwrap(), 3);

        // - Ensure that the chain does not advance due to init failing
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell);
        assert_eq!(source_chain.await.len().unwrap(), 3);

        // - Ensure idempotence of the above
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell);
        assert_eq!(source_chain.await.len().unwrap(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_during_init_one_zome_unimplemented_one_fails() {
        let zome_fail = InlineZome::new_unique(vec![]).callback("init", |api, _: ()| {
            api.create(CreateInput::new(
                EntryDefId::CapGrant,
                Entry::CapGrant(CapGrantEntry {
                    tag: "".into(),
                    access: ().into(),
                    functions: vec![("no-init".into(), "xxx".into())].into_iter().collect(),
                }),
                ChainTopOrdering::default(),
            ))?;
            Ok(InitCallbackResult::Fail("reason".into()))
        });
        let zome_no_init = crate::conductor::conductor::tests::simple_create_entry_zome();

        let (dna, _) = SweetDnaFile::unique_from_inline_zomes(vec![
            ("no-init", zome_no_init),
            ("fail", zome_fail),
        ])
        .await
        .unwrap();

        let mut conductor = SweetConductor::from_standard_config().await;
        let app = conductor.setup_app("app", &[dna]).await.unwrap();
        let (cell,) = app.into_tuple();
        let zome = cell.zome("no-init");

        assert_eq!(get_chain(&cell).await.len().unwrap(), 3);

        // - Ensure that the chain does not advance due to init failing
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell);
        assert_eq!(source_chain.await.len().unwrap(), 3);

        // - Ensure idempotence of the above
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell);
        assert_eq!(source_chain.await.len().unwrap(), 3);
    }
}
