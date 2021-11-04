#![allow(missing_docs)]

use super::host_fn_caller::HostFnCaller;
use super::install_app;
use super::setup_app_inner;
use crate::conductor::api::CellConductorApi;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::interface::SignalBroadcaster;
use crate::conductor::ConductorHandle;
use crate::core::queue_consumer::QueueTriggers;
use crate::core::ribosome::real_ribosome::RealRibosome;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::actor::HolochainP2pRefToDna;
use holochain_p2p::HolochainP2pDna;
use holochain_serialized_bytes::SerializedBytes;
use holochain_state::{prelude::test_environments, test_utils::TestEnvs};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p::KitsuneP2pConfig;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::TryFrom;

/// A "factory" for HostFnCaller, which will produce them when given a ZomeName
pub struct CellHostFnCaller {
    pub cell_id: CellId,
    pub env: EnvWrite,
    pub cache: EnvWrite,
    pub ribosome: RealRibosome,
    pub network: HolochainP2pDna,
    pub keystore: MetaLairClient,
    pub signal_tx: SignalBroadcaster,
    pub triggers: QueueTriggers,
    pub cell_conductor_api: CellConductorApi,
}

impl CellHostFnCaller {
    pub async fn new(cell_id: &CellId, handle: &ConductorHandle, dna_file: &DnaFile) -> Self {
        let env = handle.get_cell_env(cell_id).unwrap();
        let cache = handle.get_cache_env(cell_id).unwrap();
        let network = handle.holochain_p2p().to_dna(cell_id.dna_hash().clone());
        let triggers = handle.get_cell_triggers(cell_id).unwrap();
        let cell_conductor_api = CellConductorApi::new(handle.clone(), cell_id.clone());

        let ribosome = RealRibosome::new(dna_file.clone());
        let signal_tx = handle.signal_broadcaster().await;
        CellHostFnCaller {
            cell_id: cell_id.clone(),
            env,
            cache,
            ribosome,
            network,
            keystore,
            signal_tx,
            triggers,
            cell_conductor_api,
        }
    }

    /// Create a HostFnCaller for a specific zome and call
    pub fn get_api<I: Into<ZomeName>>(&self, zome_name: I) -> HostFnCaller {
        let zome_name: ZomeName = zome_name.into();
        let zome_path = (self.cell_id.clone(), zome_name).into();
        let call_zome_handle = self.cell_conductor_api.clone().into_call_zome_handle();
        HostFnCaller {
            env: self.env.clone(),
            cache: self.cache.clone(),
            ribosome: self.ribosome.clone(),
            zome_path,
            network: self.network.clone(),
            keystore: self.keystore.clone(),
            signal_tx: self.signal_tx.clone(),
            call_zome_handle,
        }
    }
}

/// Everything you need to run a test that uses the conductor
// TODO: refactor this to be the "Test Conductor" wrapper
pub struct ConductorTestData {
    _envs: TestEnvs,
    handle: ConductorHandle,
    cell_apis: BTreeMap<CellId, CellHostFnCaller>,
}

impl ConductorTestData {
    pub async fn new(
        envs: TestEnvs,
        dna_files: Vec<DnaFile>,
        agents: Vec<AgentPubKey>,
        network_config: KitsuneP2pConfig,
    ) -> (Self, HashMap<DnaHash, Vec<CellId>>) {
        let num_agents = agents.len();
        let num_dnas = dna_files.len();
        let mut cells = Vec::with_capacity(num_dnas * num_agents);
        let mut cell_id_by_dna_file = Vec::with_capacity(num_dnas);
        for (i, dna_file) in dna_files.iter().enumerate() {
            let mut cell_ids = Vec::with_capacity(num_agents);
            for (j, agent_id) in agents.iter().enumerate() {
                let cell_id = CellId::new(dna_file.dna_hash().to_owned(), agent_id.clone());
                cells.push((
                    InstalledCell::new(cell_id.clone(), format!("agent-{}-{}", i, j)),
                    None,
                ));
                cell_ids.push(cell_id);
            }
            cell_id_by_dna_file.push((dna_file, cell_ids));
        }

        let (_envs, _app_api, handle) = setup_app_inner(
            envs,
            vec![("test_app", cells)],
            dna_files.clone(),
            Some(network_config),
        )
        .await;

        let mut cell_apis = BTreeMap::new();

        for (dna_file, cell_ids) in cell_id_by_dna_file.iter() {
            for cell_id in cell_ids {
                cell_apis.insert(
                    cell_id.clone(),
                    CellHostFnCaller::new(cell_id, &handle, dna_file).await,
                );
            }
        }

        let this = Self {
            _envs,
            // app_api,
            handle,
            cell_apis,
        };
        let installed = cell_id_by_dna_file
            .into_iter()
            .map(|(dna_file, cell_ids)| (dna_file.dna_hash().clone(), cell_ids))
            .collect();
        (this, installed)
    }

    /// Create a new conductor and test data
    pub async fn two_agents(zomes: Vec<TestWasm>, with_bob: bool) -> Self {
        Self::two_agents_inner(zomes, with_bob, None).await
    }

    /// New test data that creates a conductor using a custom network config
    pub async fn with_network_config(
        zomes: Vec<TestWasm>,
        with_bob: bool,
        network: KitsuneP2pConfig,
    ) -> Self {
        Self::two_agents_inner(zomes, with_bob, Some(network)).await
    }

    async fn two_agents_inner(
        zomes: Vec<TestWasm>,
        with_bob: bool,
        network: Option<KitsuneP2pConfig>,
    ) -> Self {
        let dna_file = DnaFile::new(
            DnaDef {
                name: "conductor_test".to_string(),
                uid: "ba1d046d-ce29-4778-914b-47e6010d2faf".to_string(),
                properties: SerializedBytes::try_from(()).unwrap(),
                zomes: zomes.clone().into_iter().map(Into::into).collect(),
            },
            zomes.into_iter().map(Into::into),
        )
        .await
        .unwrap();

        let mut agents = vec![fake_agent_pubkey_1()];
        if with_bob {
            agents.push(fake_agent_pubkey_2());
        }

        let (this, _) = Self::new(
            test_environments(),
            vec![dna_file],
            agents,
            network.unwrap_or_default(),
        )
        .await;

        this
    }

    /// Shutdown the conductor
    pub async fn shutdown_conductor(&mut self) {
        let shutdown = self.handle.take_shutdown_handle().unwrap();
        self.handle.shutdown();
        shutdown.await.unwrap().unwrap();
    }

    /// Bring bob online if he isn't already
    pub async fn bring_bob_online(&mut self) {
        let dna_file = self.alice_call_data().ribosome.dna_file().clone();
        if self.bob_call_data().is_none() {
            let bob_agent_id = fake_agent_pubkey_2();
            let bob_cell_id = CellId::new(dna_file.dna_hash().clone(), bob_agent_id.clone());
            let bob_installed_cell = InstalledCell::new(bob_cell_id.clone(), "bob_handle".into());
            let cell_data = vec![(bob_installed_cell, None)];
            install_app("bob_app", cell_data, vec![dna_file.clone()], self.handle()).await;
            self.cell_apis.insert(
                bob_cell_id.clone(),
                CellHostFnCaller::new(&bob_cell_id, &self.handle(), &dna_file).await,
            );
        }
    }

    pub fn handle(&self) -> ConductorHandle {
        self.handle.clone()
    }

    #[allow(clippy::iter_nth_zero)]
    pub fn alice_call_data(&self) -> &CellHostFnCaller {
        match self.cell_apis.values().len() {
            0 => unreachable!(),
            1 => self.cell_apis.values().next().unwrap(),
            2 => self.cell_apis.values().nth(1).unwrap(),
            _ => unimplemented!(),
        }
    }

    pub fn bob_call_data(&self) -> Option<&CellHostFnCaller> {
        match self.cell_apis.values().len() {
            0 => unreachable!(),
            1 => None,
            2 => self.cell_apis.values().next(),
            _ => unimplemented!(),
        }
    }

    #[allow(clippy::iter_nth_zero)]
    pub fn alice_call_data_mut(&mut self) -> &mut CellHostFnCaller {
        let key = self.cell_apis.keys().nth(0).unwrap().clone();
        self.cell_apis.get_mut(&key).unwrap()
    }

    pub fn get_cell(&mut self, cell_id: &CellId) -> Option<&mut CellHostFnCaller> {
        self.cell_apis.get_mut(cell_id)
    }
}
