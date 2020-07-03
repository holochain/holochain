use crate::{
    conductor::{
        api::error::ConductorApiResult,
        config::AdminInterfaceConfig,
        error::{ConductorResult, CreateAppError},
        manager::TaskManagerRunHandle,
        state::ConductorState,
    },
    core::{
        ribosome::ZomeCallInvocation,
        state::{dht_op_integration::IntegrationQueueValue, workspace::Workspace},
        workflow::ZomeCallInvocationResult,
    },
    fixt::{DnaFileFixturator, SignatureFixturator},
};
use fallible_iterator::FallibleIterator;
use fixt::prelude::*;
use holo_hash::{
    AgentPubKeyFixturator, DhtOpHashFixturator, DnaHash, DnaHashFixturator, HeaderHashFixturator,
};
use holochain_keystore::KeystoreSender;
use holochain_p2p::actor::HolochainP2pRefToCell;
use holochain_state::{
    env::{EnvironmentWrite, ReadManager},
    test_utils::{test_conductor_env, TestEnvironment},
};
use holochain_types::{
    app::{AppId, InstalledApp, InstalledCell, MembraneProof},
    autonomic::AutonomicCue,
    cell::CellId,
    dht_op::DhtOp,
    dna::DnaFile,
    header,
    test_utils::fake_cell_id,
    Timestamp,
};
use std::sync::Arc;

#[derive(Clone)]
struct TestH;

#[async_trait::async_trait]
impl crate::conductor::handle::ConductorHandleT for TestH {
    async fn check_running(&self) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn add_admin_interfaces(
        self: Arc<Self>,
        _configs: Vec<AdminInterfaceConfig>,
    ) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn add_app_interface(self: Arc<Self>, _port: u16) -> ConductorResult<u16> {
        unimplemented!()
    }
    async fn install_dna(&self, _dna: DnaFile) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn list_dnas(&self) -> ConductorResult<Vec<DnaHash>> {
        unimplemented!()
    }
    async fn get_dna(&self, _hash: &DnaHash) -> Option<DnaFile> {
        Some(fixt!(DnaFile))
    }
    async fn add_dnas(&self) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn dispatch_holochain_p2p_event(
        &self,
        _cell_id: &CellId,
        _event: holochain_p2p::event::HolochainP2pEvent,
    ) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn call_zome(
        &self,
        _invocation: ZomeCallInvocation,
    ) -> ConductorApiResult<ZomeCallInvocationResult> {
        unimplemented!()
    }
    async fn autonomic_cue(&self, _cue: AutonomicCue, _cell_id: &CellId) -> ConductorApiResult<()> {
        unimplemented!()
    }
    async fn get_arbitrary_admin_websocket_port(&self) -> Option<u16> {
        unimplemented!()
    }
    async fn take_shutdown_handle(&self) -> Option<TaskManagerRunHandle> {
        unimplemented!()
    }
    async fn shutdown(&self) {
        unimplemented!()
    }
    fn keystore(&self) -> &KeystoreSender {
        unimplemented!()
    }
    fn holochain_p2p(&self) -> &holochain_p2p::HolochainP2pRef {
        unimplemented!()
    }
    async fn install_app(
        self: Arc<Self>,
        _app_id: AppId,
        _cell_data_with_proofs: Vec<(InstalledCell, Option<MembraneProof>)>,
    ) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn setup_cells(self: Arc<Self>) -> ConductorResult<Vec<CreateAppError>> {
        unimplemented!()
    }
    async fn activate_app(&self, _app_id: AppId) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn deactivate_app(&self, _app_id: AppId) -> ConductorResult<()> {
        unimplemented!()
    }
    async fn dump_cell_state(&self, _cell_id: &CellId) -> ConductorApiResult<String> {
        unimplemented!()
    }
    async fn get_app_info(&self, _app_id: &AppId) -> ConductorResult<Option<InstalledApp>> {
        unimplemented!()
    }
    #[cfg(test)]
    async fn get_cell_env(&self, _cell_id: &CellId) -> ConductorApiResult<EnvironmentWrite> {
        unimplemented!()
    }
    #[cfg(test)]
    async fn get_state_from_handle(&self) -> ConductorApiResult<ConductorState> {
        unimplemented!()
    }
}

#[tokio::test(threaded_scheduler)]
async fn test_cell_handle_publish() {
    let TestEnvironment { env, tmpdir } = test_conductor_env();
    let keystore = env.keystore().clone();
    let (holochain_p2p, _p2p_evt) = holochain_p2p::spawn_holochain_p2p().await.unwrap();
    let cell_id = fake_cell_id("dr. cell");

    let dna = fixt!(DnaHash);
    let agents = AgentPubKeyFixturator::new(Unpredictable)
        .take(2)
        .collect::<Vec<_>>();

    let holochain_p2p_cell = holochain_p2p.to_cell(dna.clone(), agents[0].clone());

    let path = tmpdir.path().to_path_buf();

    super::Cell::genesis(
        cell_id.clone(),
        Arc::new(TestH),
        path.clone(),
        keystore.clone(),
        None,
    )
    .await
    .unwrap();

    let cell = super::Cell::create(cell_id, Arc::new(TestH), path, keystore, holochain_p2p_cell)
        .await
        .unwrap();

    let header_hash = fixt!(HeaderHash);
    let op_hash = fixt!(DhtOpHash);
    let sig = fixt!(Signature);
    let header = header::Header::Dna(header::Dna {
        author: agents[0].clone(),
        timestamp: Timestamp::now(),
        hash: dna.clone(),
        header_seq: 42,
    });
    let op = DhtOp::StoreElement(sig, header, None);

    cell.handle_publish(
        agents[0].clone(),
        true,
        header_hash.into(),
        vec![(op_hash, op.clone())],
    )
    .await
    .unwrap();

    let env_ref = cell.state_env.guard().await;
    let reader = env_ref.reader().expect("Could not create LMDB reader");
    let workspace = crate::core::workflow::produce_dht_ops_workflow::ProduceDhtOpsWorkspace::new(
        &reader, &env_ref,
    )
    .expect("Could not create Workspace");

    let res = workspace
        .integration_queue
        .iter()
        .unwrap()
        .collect::<Vec<_>>()
        .unwrap();
    let (_, last) = &res[res.len() - 1];

    matches::assert_matches!(
        last,
        IntegrationQueueValue {
            op: DhtOp::StoreElement(
                _,
                header::Header::Dna(
                    header::Dna {
                        hash,
                        ..
                    }
                ),
                _,
            ),
            ..
        } if hash == &dna
    );
}
