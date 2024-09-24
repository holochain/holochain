use super::error::WorkflowResult;
use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::guest_callback::init::InitHostAccess;
use crate::core::ribosome::guest_callback::init::InitInvocation;
use crate::core::ribosome::guest_callback::init::InitResult;
use crate::core::ribosome::guest_callback::post_commit::send_post_commit;
use crate::core::ribosome::RibosomeT;
use derive_more::Constructor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::HolochainP2pDna;
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_types::prelude::*;
use holochain_zome_types::action::builder;
use tokio::sync::broadcast;

#[derive(Constructor)]
pub struct InitializeZomesWorkflowArgs<Ribosome>
where
    Ribosome: RibosomeT + 'static,
{
    pub ribosome: Ribosome,
    pub conductor_handle: ConductorHandle,
    pub signal_tx: broadcast::Sender<Signal>,
    pub cell_id: CellId,
    pub integrate_dht_ops_trigger: TriggerSender,
}

impl<Ribosome> InitializeZomesWorkflowArgs<Ribosome>
where
    Ribosome: RibosomeT + 'static,
{
    pub fn dna_def(&self) -> &DnaDef {
        self.ribosome.dna_def().as_content()
    }
}

// #[cfg_attr(feature = "instrument", tracing::instrument(skip(network, keystore, workspace, args)))]
pub async fn initialize_zomes_workflow<Ribosome>(
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult>
where
    Ribosome: RibosomeT + Clone + 'static,
{
    let conductor_handle = args.conductor_handle.clone();
    let coordinators = args.ribosome.dna_def().get_all_coordinators();
    let integrate_dht_ops_trigger = args.integrate_dht_ops_trigger.clone();
    let signal_tx = args.signal_tx.clone();
    let result =
        initialize_zomes_workflow_inner(workspace.clone(), network.clone(), keystore.clone(), args)
            .await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // only commit if the result was successful
    if result == InitResult::Pass {
        let flushed_actions = workspace.source_chain().flush(&network).await?;

        send_post_commit(
            conductor_handle,
            workspace,
            network,
            keystore,
            flushed_actions,
            coordinators,
            signal_tx,
        )
        .await?;

        // Any ops that were moved to the dht_db as part of the flush but had dependencies will need to be integrated.
        integrate_dht_ops_trigger.trigger(&"initialize_zomes_workflow");
    }
    Ok(result)
}

async fn initialize_zomes_workflow_inner<Ribosome>(
    workspace: SourceChainWorkspace,
    network: HolochainP2pDna,
    keystore: MetaLairClient,
    args: InitializeZomesWorkflowArgs<Ribosome>,
) -> WorkflowResult<InitResult>
where
    Ribosome: RibosomeT + 'static,
{
    let dna_def = args.dna_def().clone();
    let InitializeZomesWorkflowArgs {
        ribosome,
        conductor_handle,
        signal_tx,
        cell_id,
        ..
    } = args;
    let call_zome_handle =
        CellConductorApi::new(conductor_handle.clone(), cell_id.clone()).into_call_zome_handle();
    let dpki = conductor_handle.running_services().dpki;

    // Call the init callback
    let result = {
        let host_access = InitHostAccess::new(
            workspace.clone().into(),
            keystore,
            dpki,
            network.clone(),
            signal_tx,
            call_zome_handle,
        );
        let invocation = InitInvocation { dna_def };
        ribosome.run_init(host_access, invocation).await?
    };

    // Insert the init marker
    // FIXME: For some reason if we don't spawn here
    // this future never gets polled again.
    let ws = workspace.clone();

    tokio::task::spawn(async move {
        ws.source_chain()
            .put(
                builder::InitZomesComplete {},
                None,
                ChainTopOrdering::Strict,
            )
            .await
    })
    .await??;

    // TODO: Validate scratch items
    super::inline_validation(workspace, network, conductor_handle, ribosome).await?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::conductor::Conductor;
    use crate::core::ribosome::guest_callback::validate::ValidateResult;
    use crate::core::ribosome::MockRibosomeT;
    use crate::fixt::DnaDefFixturator;
    use crate::fixt::MetaLairClientFixturator;
    use crate::sweettest::*;
    use crate::test_utils::fake_genesis;
    use ::fixt::prelude::*;
    use holochain_keystore::test_keystore;
    use holochain_p2p::HolochainP2pDnaFixturator;
    use holochain_state::prelude::*;
    use holochain_state::test_utils::test_db_dir;
    use holochain_types::db_cache::DhtDbQueryCache;
    use holochain_types::inline_zome::InlineZomeSet;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;

    async fn get_chain(cell: &SweetCell, keystore: MetaLairClient) -> SourceChain {
        SourceChain::new(
            cell.authored_db().clone(),
            cell.dht_db().clone(),
            DhtDbQueryCache::new(cell.dht_db().clone().into()),
            keystore,
            cell.agent_pubkey().clone(),
        )
        .await
        .unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn adds_init_marker() {
        let test_db = test_authored_db();
        let test_cache = test_cache_db();
        let test_dht = test_dht_db();
        let keystore = test_keystore();
        let db = test_db.to_db();
        let author = fake_agent_pubkey_1();

        // Genesis
        fake_genesis(db.clone(), test_dht.to_db(), keystore.clone())
            .await
            .unwrap();

        let dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();
        let dna_def_hashed = DnaDefHashed::from_content_sync(dna_def.clone());

        let workspace = SourceChainWorkspace::new(
            db.clone(),
            test_dht.to_db(),
            DhtDbQueryCache::new(test_dht.to_db().into()),
            test_cache.to_db(),
            keystore,
            author.clone(),
            Arc::new(dna_def),
        )
        .await
        .unwrap();
        let mut ribosome = MockRibosomeT::new();

        // Setup the ribosome mock
        ribosome
            .expect_run_init()
            .returning(move |_workspace, _invocation| Ok(InitResult::Pass));
        ribosome
            .expect_run_validate()
            .returning(move |_, _| Ok(ValidateResult::Valid));
        ribosome
            .expect_dna_def()
            .return_const(dna_def_hashed.clone());

        let db_dir = test_db_dir();
        let conductor_handle = Conductor::builder()
            .config(SweetConductorConfig::standard().no_dpki().into())
            .with_data_root_path(db_dir.path().to_path_buf().into())
            .test(&[])
            .await
            .unwrap();
        let integrate_dht_ops_trigger = TriggerSender::new();

        let args = InitializeZomesWorkflowArgs {
            ribosome,
            conductor_handle,
            signal_tx: broadcast::channel(1).0,
            cell_id: CellId::new(dna_def_hashed.to_hash(), author.clone()),
            integrate_dht_ops_trigger: integrate_dht_ops_trigger.0.clone(),
        };
        let keystore = fixt!(MetaLairClient);
        let network = fixt!(HolochainP2pDna);

        initialize_zomes_workflow_inner(workspace.clone(), network, keystore, args)
            .await
            .unwrap();

        // Check init is added to the workspace
        let scratch = workspace.source_chain().snapshot().unwrap();
        assert_matches!(
            scratch.actions().next().unwrap().action(),
            Action::InitZomesComplete(_)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_during_init() {
        let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
        let mut conductor = SweetConductor::isolated_singleton().await;
        let keystore = conductor.keystore();
        let app = conductor.setup_app("app", [&dna]).await.unwrap();
        let (cell,) = app.into_tuple();
        let zome = cell.zome("create_entry");

        assert_eq!(get_chain(&cell, keystore.clone()).await.len().unwrap(), 3);
        assert_eq!(
            get_chain(&cell, keystore.clone())
                .await
                .query(Default::default())
                .await
                .unwrap()
                .len(),
            3
        );

        let _: ActionHash = conductor.call(&zome, "create_entry", ()).await;

        let source_chain = get_chain(&cell, keystore.clone()).await;
        // - Ensure that the InitZomesComplete record got committed after the
        //   record committed during init()
        assert_matches!(
            source_chain.query(Default::default()).await.unwrap()[4].action(),
            Action::InitZomesComplete(_)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_during_init_one_zome_passes_one_fails() {
        let (dna, _, _) =
            SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create, TestWasm::InitFail]).await;
        let mut conductor = SweetConductor::isolated_singleton().await;
        let keystore = conductor.keystore();
        let app = conductor.setup_app("app", [&dna]).await.unwrap();
        let (cell,) = app.into_tuple();
        let zome = cell.zome("create_entry");

        assert_eq!(get_chain(&cell, keystore.clone()).await.len().unwrap(), 3);

        // - Ensure that the chain does not advance due to init failing
        let r: Result<ActionHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell, keystore.clone());
        assert_eq!(source_chain.await.len().unwrap(), 3);

        // - Ensure idempotence of the above
        let r: Result<ActionHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell, keystore.clone());
        assert_eq!(source_chain.await.len().unwrap(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_during_init_one_zome_unimplemented_one_fails() {
        let zome_fail = SweetInlineZomes::new(vec![], 0).function("init", |api, _: ()| {
            api.create(CreateInput::new(
                EntryDefLocation::CapGrant,
                EntryVisibility::Private,
                Entry::CapGrant(CapGrantEntry {
                    tag: "".into(),
                    access: ().into(),
                    functions: GrantedFunctions::Listed(
                        vec![("no-init".into(), "xxx".into())].into_iter().collect(),
                    ),
                }),
                ChainTopOrdering::default(),
            ))?;
            Ok(InitCallbackResult::Fail("reason".into()))
        });
        let zomes = InlineZomeSet::from((
            "create_entry",
            crate::conductor::conductor::tests::simple_create_entry_zome(),
        ))
        .merge(zome_fail.0);

        let (dna, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;

        let mut conductor = SweetConductor::isolated_singleton().await;
        let keystore = conductor.keystore();
        let app = conductor.setup_app("app", [&dna]).await.unwrap();
        let (cell,) = app.into_tuple();
        let zome = cell.zome("create_entry");

        assert_eq!(get_chain(&cell, keystore.clone()).await.len().unwrap(), 3);

        // - Ensure that the chain does not advance due to init failing
        let r: Result<ActionHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell, keystore.clone());
        assert_eq!(source_chain.await.len().unwrap(), 3);

        // - Ensure idempotence of the above
        let r: Result<ActionHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(&cell, keystore.clone());
        assert_eq!(source_chain.await.len().unwrap(), 3);
    }
}
