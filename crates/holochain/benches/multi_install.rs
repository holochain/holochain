use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use holochain::sweettest::*;
use holochain::test_utils::consistency_envs;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::InterfaceDriver;
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_types::prelude::InstallAppBundlePayload;
use holochain_types::prelude::{AppBundleSource, InstalledCell};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::CellId;
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

// TODO: Produce a high data version of this bench.
// TODO: Add profile function to queries that need optimizing.
// TODO: Research indexing.

criterion_group!(benches, multi_install);

criterion_main!(benches);

fn multi_install(bench: &mut Criterion) {
    observability::test_run().ok();
    let mut group = bench.benchmark_group("multi_install");
    group.sample_size(
        std::env::var_os("BENCH_SAMPLE_SIZE")
            .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
            .unwrap_or(100),
    );
    let runtime = rt();

    let mut producers = runtime.block_on(setup());
    // if let Some(n) = std::env::var_os("BENCH_NUM_OPS") {
    //     let num_ops = n.to_string_lossy().parse::<usize>().unwrap();
    // runtime.block_on(async {
    //     producer.fill(num_ops).await;
    //     let cells = vec![&consumer.cell, &producer.cell];
    //     let num_tries = std::env::var_os("BENCH_NUM_WAITS")
    //         .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
    //         .unwrap_or(100);
    //     holochain::test_utils::consistency(
    //         &cells,
    //         num_tries,
    //         std::time::Duration::from_millis(500),
    //     )
    //     .await;
    //     holochain_state::prelude::dump_tmp(consumer.cell.env());
    // });
    // }
    // runtime.spawn(async move {
    //     producer.run().await;
    //     producer.conductor.shutdown_and_wait().await;
    // });
    group.bench_function(BenchmarkId::new("test", format!("test")), |b| {
        b.iter(|| {
            runtime.block_on(async { producers.run().await });
        });
    });
    runtime.block_on(async move {
        for c in producers.conductors {
            c.shutdown_and_wait().await;
            drop(c);
        }
    });
    runtime.shutdown_background();
}

struct Producers {
    conductors: Vec<SweetConductor>,
    i: usize,
    total: usize,
}

// struct Consumer {
//     conductor: SweetConductor,
//     cell: SweetCell,
//     last: usize,
//     tx: tokio::sync::mpsc::Sender<usize>,
// }

// struct Others {
//     conductors: Vec<SweetConductor>,
//     cells: Vec<SweetCell>,
// }

impl Producers {
    async fn run(&mut self) {
        use holochain_keystore::KeystoreSenderExt;
        if self.total >= 100 {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        } else {
            for _ in 0..10 {
                let conductor = &self.conductors[self.i];
                let agent_key = conductor
                    .keystore()
                    .clone()
                    .generate_sign_keypair_from_pure_entropy()
                    .await
                    .expect("Failed to generate agent key");
                self.i += 1;
                if self.i >= self.conductors.len() {
                    self.i = 0;
                }
                self.total += 1;
                let source = AppBundleSource::Path(PathBuf::from(
                    "/home/freesig/holochain/elemental-chat/elemental-chat.happ",
                ));
                let mut membrane_proofs = HashMap::new();
                membrane_proofs.insert(
                    "elemental-chat".to_string(),
                    SerializedBytes::from(UnsafeBytes::from(vec![0])),
                );
                let payload = InstallAppBundlePayload {
                    source,
                    agent_key,
                    installed_app_id: Some(format!("ec {}", self.total)),
                    membrane_proofs,
                    uid: None,
                };
                let id = holochain::conductor::handle::ConductorHandleT::install_app_bundle(
                    conductor.inner_handle(),
                    payload,
                )
                .await
                .expect("Failed to install");
                let id = id.installed_app_id().clone();
                conductor
                    .activate_app(id)
                    .await
                    .expect("Failed to activate app");

                let errors = conductor
                    .inner_handle()
                    .setup_cells()
                    .await
                    .expect("Failed to setup cells");
                assert_eq!(0, errors.len(), "{:?}", errors);
            }
            let mut cells = Vec::new();
            for c in &self.conductors {
                let ids = c.list_cell_ids().await.expect("Failed to list cell ids");
                for id in ids {
                    cells.push(c.get_cell_env(&id).await.unwrap());
                }
            }
            let cell_refs = cells.iter().collect::<Vec<_>>();

            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
        // consistency_envs(&cell_refs, 2000, std::time::Duration::from_millis(500)).await;
    }
    async fn run_create(&mut self) {
        use holochain_keystore::KeystoreSenderExt;
        for _ in 0..10 {
            let conductor = &self.conductors[self.i];
            let agent_key = conductor
                .keystore()
                .clone()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .expect("Failed to generate agent key");
            self.i += 1;
            if self.i >= self.conductors.len() {
                self.i = 0;
            }
            self.total += 1;
            let (dna, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::ValidateValid])
                .await
                .unwrap();
            conductor
                .register_dna(dna.clone())
                .await
                .expect("Failed to install dna");
            let cell_handle = format!("{}", dna.dna_hash());
            let cell_id = CellId::new(dna.dna_hash().clone(), agent_key.clone());
            let id = format!("ec {}", self.total);
            let i_cells = vec![(InstalledCell::new(cell_id, cell_handle), None)];
            holochain::conductor::handle::ConductorHandleT::install_app(
                conductor.inner_handle(),
                id.clone(),
                i_cells,
            )
            .await
            .unwrap();
            conductor
                .activate_app(id)
                .await
                .expect("Failed to activate app");

            let errors = conductor
                .inner_handle()
                .setup_cells()
                .await
                .expect("Failed to setup cells");
            assert_eq!(0, errors.len());
        }
        // let mut cells = Vec::new();
        // for c in &self.conductors {
        //     println!("Num infos {}", c.get_agent_infos(None).await.unwrap().len());
        //     let ids = c.list_cell_ids().await.expect("Failed to list cell ids");
        //     for id in ids {
        //         cells.push(c.get_cell_env(&id).await.unwrap());
        //     }
        // }
        // let cell_refs = cells.iter().collect::<Vec<_>>();
        // consistency_envs(&cell_refs, 20, std::time::Duration::from_millis(500)).await;
        // for c in &self.conductors {
        //     println!("Num infos {:?}", c.get_agent_infos(None).await.unwrap());
        // }
        // consistency_envs(&cell_refs, 2000, std::time::Duration::from_millis(500)).await;
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn setup() -> Producers {
    let config = || {
        let mut tuning: KitsuneP2pTuningParams = KitsuneP2pTuningParams::default();
        tuning.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 10;
        tuning.gossip_peer_on_error_next_gossip_delay_ms = 1000 * 20;

        let mut network = KitsuneP2pConfig::default();
        network.tuning_params = Arc::new(tuning);
        network.bootstrap_service = Some(url2::url2!("http://127.0.0.1:3030"));
        // network.bootstrap_service = Some(url2::url2!("https://bootstrap-staging.holo.host"));
        network.transport_pool = vec![kitsune_p2p::TransportConfig::Quic {
            bind_to: None,
            override_host: None,
            override_port: None,
        }];
        ConductorConfig {
            network: Some(network),
            admin_interfaces: Some(vec![AdminInterfaceConfig {
                driver: InterfaceDriver::Websocket { port: 0 },
            }]),
            ..Default::default()
        }
    };
    let mut configs = Vec::new();
    for _ in 0..1 {
        configs.push(config());
    }
    let conductors = SweetConductorBatch::from_configs(configs.clone()).await;

    let conductors = conductors.into_inner().into_iter();

    Producers {
        conductors: conductors.collect(),
        i: 0,
        total: 0,
    }
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
