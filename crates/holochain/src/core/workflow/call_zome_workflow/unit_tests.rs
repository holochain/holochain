use crate::{
    conductor::api::CellConductorApi,
    core::{
        queue_consumer::TriggerSender,
        ribosome::{real_ribosome::RealRibosome, ZomeCallInvocation},
        workflow::{call_zome_workflow, CallZomeWorkflowArgs},
    },
    sweettest::{SweetConductor, SweetDnaFile},
    test_utils::new_zome_call_params,
};
use ::fixt::fixt;
use hdk::prelude::DnaId;
use holo_hash::fixt::ActionHashFixturator;
use holochain_keystore::MetaLairClient;
use holochain_p2p::{actor::MockHcP2p, DynHolochainP2pDna, HolochainP2pDna};
use holochain_state::host_fn_workspace::SourceChainWorkspace;
use holochain_wasm_test_utils::TestWasm;
use kitsune2_api::DhtArc;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn trigger_integration_workflow_after_creating_ops() {
    let mut hc_p2p = MockHcP2p::new();
    hc_p2p
        .expect_target_arcs()
        .returning(|_| Box::pin(async { Ok(vec![DhtArc::FULL]) }));
    let TestCase {
        workspace,
        network,
        keystore,
        args,
        _conductor,
    } = TestCase::new(hc_p2p, "create", ()).await;
    let (trigger_publish_dht_ops, _) = TriggerSender::new();
    let (trigger_integrate_dht_ops, mut integrate_dht_ops_rx) = TriggerSender::new();
    let (trigger_countersigning, _) = TriggerSender::new();

    let _ = call_zome_workflow(
        workspace,
        network,
        keystore,
        args,
        trigger_publish_dht_ops,
        trigger_integrate_dht_ops,
        trigger_countersigning,
    )
    .await
    .unwrap()
    .unwrap();
    // Assert the integration workflow has been triggered.
    integrate_dht_ops_rx.try_recv().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn integration_workflow_is_not_triggered_when_no_data_has_been_created() {
    let mut hc_p2p = MockHcP2p::new();
    hc_p2p
        .expect_target_arcs()
        .returning(|_| Box::pin(async { Ok(vec![DhtArc::FULL]) }));
    hc_p2p
        .expect_authority_for_hash()
        .returning(|_, _| Box::pin(async { Ok(true) }));
    let TestCase {
        workspace,
        network,
        keystore,
        args,
        _conductor,
        // Zome call to get action that does not exist.
    } = TestCase::new(hc_p2p, "reed", fixt!(ActionHash)).await;
    let (trigger_publish_dht_ops, _) = TriggerSender::new();
    let (trigger_integrate_dht_ops, mut integrate_dht_ops_rx) = TriggerSender::new();
    let (trigger_countersigning, _) = TriggerSender::new();

    let _ = call_zome_workflow(
        workspace,
        network,
        keystore,
        args,
        trigger_publish_dht_ops,
        trigger_integrate_dht_ops,
        trigger_countersigning,
    )
    .await
    .unwrap()
    .unwrap();
    // Fail the test if the integration workflow has been triggered.
    assert!(integrate_dht_ops_rx.try_recv().is_none());
}

struct TestCase {
    workspace: SourceChainWorkspace,
    network: DynHolochainP2pDna,
    keystore: MetaLairClient,
    args: CallZomeWorkflowArgs<RealRibosome>,
    _conductor: SweetConductor,
}

impl TestCase {
    pub async fn new<P>(hc_p2p: MockHcP2p, zome_call_fn_name: &str, zome_call_payload: P) -> Self
    where
        P: serde::Serialize + std::fmt::Debug,
    {
        let mut conductor = SweetConductor::from_standard_config().await;
        let (dna_file, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Crd]).await;
        let app = conductor.setup_app("", &[dna_file]).await.unwrap();
        let dna_hash = app.cells()[0].dna_hash().clone();
        let agent = app.agent().clone();
        let dna_id = DnaId::new(dna_hash.clone(), agent.clone());
        let workspace = SourceChainWorkspace::new(
            conductor
                .get_or_create_authored_db(&dna_hash, agent.clone())
                .unwrap(),
            conductor.get_dht_db(&dna_hash).unwrap(),
            conductor.get_cache_db(&dna_id).await.unwrap(),
            conductor.keystore(),
            agent.clone(),
        )
        .await
        .unwrap();
        // Get action that does not exist.
        let zome_call_params = new_zome_call_params(
            &dna_id,
            zome_call_fn_name,
            zome_call_payload,
            TestWasm::Crd.coordinator_zome_name(),
        )
        .unwrap();
        let invocation = ZomeCallInvocation::try_from_params(
            Arc::new(CellConductorApi::new(conductor.clone(), dna_id.clone())),
            zome_call_params,
        )
        .await
        .unwrap();
        let (signal_tx, _signal_rx) = tokio::sync::broadcast::channel(1);
        let args = CallZomeWorkflowArgs {
            dna_id,
            ribosome: conductor.get_ribosome(&dna_hash).unwrap(),
            invocation,
            signal_tx: signal_tx.clone(),
            conductor_handle: conductor.clone(),
            is_root_zome_call: true,
        };
        let hc_p2p = Arc::new(hc_p2p);
        let network = Arc::new(HolochainP2pDna::new(hc_p2p, dna_hash, None));

        Self {
            workspace,
            network,
            keystore: conductor.keystore(),
            args,
            _conductor: conductor,
        }
    }
}
