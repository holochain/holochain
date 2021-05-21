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
use holochain_types::prelude::*;
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

    // only commit if the result was successful
    if result == InitResult::Pass {
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
    use crate::sweettest::*;
    use ::fixt::prelude::*;
    use fixt::Unpredictable;
    use holochain_lmdb::{env::EnvironmentWrite, test_utils::test_cell_env};
    use holochain_p2p::HolochainP2pCellFixturator;
    use holochain_state::prelude::SourceChainBuf;
    use holochain_wasm_test_utils::TestWasm;
    use matches::assert_matches;

    fn get_chain(env: &EnvironmentWrite) -> SourceChainBuf {
        SourceChainBuf::new(env.clone().into()).unwrap()
    }

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
        initialize_zomes_workflow(workspace, network, keystore, env.clone().into(), args)
            .await
            .unwrap();

        // - Ensure that the InitZomesComplete element got committed
        let source_chain = SourceChainBuf::new(env.into()).unwrap();
        assert_matches!(
            source_chain.get_at_index(3).unwrap().unwrap().header(),
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

        assert_eq!(get_chain(cell.env()).len(), 3);

        let _: HeaderHash = conductor.call(&zome, "create_entry", ()).await;

        let source_chain = SourceChainBuf::new(cell.env().clone().into()).unwrap();
        // - Ensure that the InitZomesComplete element got committed after the
        //   element committed during init()
        assert_matches!(
            source_chain.get_at_index(4).unwrap().unwrap().header(),
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

        assert_eq!(get_chain(cell.env()).len(), 3);

        // - Ensure that the chain does not advance due to init failing
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(cell.env());
        assert_eq!(source_chain.len(), 3);

        // - Ensure idempotence of the above
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(cell.env());
        assert_eq!(source_chain.len(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_during_init_one_zome_unimplemented_one_fails() {
        let zome_fail = InlineZome::new_unique(vec![]).callback("init", |api, _: ()| {
            api.create(EntryWithDefId::new(
                EntryDefId::CapGrant,
                Entry::CapGrant(CapGrantEntry {
                    tag: "".into(),
                    access: ().into(),
                    functions: vec![("no-init".into(), "xxx".into())].into_iter().collect(),
                }),
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

        assert_eq!(get_chain(cell.env()).len(), 3);

        // - Ensure that the chain does not advance due to init failing
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(cell.env());
        assert_eq!(source_chain.len(), 3);

        // - Ensure idempotence of the above
        let r: Result<HeaderHash, _> = conductor.call_fallible(&zome, "create_entry", ()).await;
        assert!(r.is_err());
        let source_chain = get_chain(cell.env());
        assert_eq!(source_chain.len(), 3);
    }
}
