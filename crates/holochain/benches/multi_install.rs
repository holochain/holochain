use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use holochain::sweettest::*;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::InterfaceDriver;
use holochain_serialized_bytes::{SerializedBytes, UnsafeBytes};
use holochain_types::prelude::InstallAppBundlePayload;
use holochain_types::prelude::{AppBundleSource, DnaFile};
use kitsune_p2p::KitsuneP2pConfig;
use kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams;
use reqwest::Client;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

criterion_group!(benches, multi_install);

criterion_main!(benches);

fn multi_install(bench: &mut Criterion) {
    observability::test_run().ok();
    let client = reqwest::Client::new();
    let num_machines = std::env::var_os("BENCH_NUM_MACHINES")
        .and_then(|s| s.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(1);
    let num_conductors = std::env::var_os("BENCH_NUM_CONDUCTORS")
        .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
        .unwrap_or(1);
    let url = std::env::var_os("BENCH_BOOTSTRAP")
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or("http://127.0.0.1:3030".to_string());

    let mut group = bench.benchmark_group("multi_install");
    group.sample_size(
        std::env::var_os("BENCH_SAMPLE_SIZE")
            .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
            .unwrap_or(100),
    );
    // group.sampling_mode(criterion::SamplingMode::Flat);
    // group.warm_up_time(std::time::Duration::from_millis(1));
    let runtime = rt();

    runtime.block_on(async {
        clear(&client, &url).await;
        sync(&client, num_machines, &url).await;
    });

    let mut producers = runtime.block_on(setup(num_conductors, &url));
    group.bench_function(BenchmarkId::new("test", format!("install")), |b| {
        b.iter(|| {
            runtime.block_on(async { producers.run().await });
        });
    });
    runtime.block_on(async { producers.consistency(&client, num_machines, &url).await });
    group.sample_size(10);
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
    test_dna: DnaFile,
    inline_dna: DnaFile,
}

impl Producers {
    async fn run(&mut self) {
        use holochain_keystore::KeystoreSenderExt;
        if self.total > 300 {
            return;
        }
        let start = std::time::Instant::now();
        for _ in 0..1 {
            let s = std::time::Instant::now();
            let conductor = &self.conductors[self.i];
            let agent_key = conductor
                .keystore()
                .clone()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .expect("Failed to generate agent key");
            if s.elapsed().as_millis() > 500 {
                dbg!(s.elapsed());
            }
            let s = std::time::Instant::now();
            self.i += 1;
            if self.i >= self.conductors.len() {
                self.i = 0;
            }
            self.total += 1;
            let source = AppBundleSource::Path(PathBuf::from(
                "/home/freesig/holochain/elemental-chat/elemental-chat.happ",
            ));

            if s.elapsed().as_millis() > 500 {
                dbg!(s.elapsed());
            }
            let s = std::time::Instant::now();
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
            if s.elapsed().as_millis() > 500 {
                dbg!(s.elapsed());
            }
            let s = std::time::Instant::now();
            let id = id.installed_app_id().clone();
            conductor
                .activate_app(id)
                .await
                .expect("Failed to activate app");

            if s.elapsed().as_millis() > 500 {
                dbg!(s.elapsed());
            }
            let s = std::time::Instant::now();
            let errors = conductor
                .inner_handle()
                .setup_cells()
                .await
                .expect("Failed to setup cells");
            assert_eq!(0, errors.len(), "{:?}", errors);
            if s.elapsed().as_millis() > 500 {
                dbg!(s.elapsed());
            }
        }
        println!("{}:{:?}", self.total, start.elapsed());
    }

    async fn run_test_wasm(&mut self) {
        use holochain_keystore::KeystoreSenderExt;
        if self.total > 300 {
            return;
        }
        let start = std::time::Instant::now();
        for _ in 0..1 {
            let len = self.conductors.len();
            let conductor = self.conductors.get_mut(self.i).unwrap();
            let agent_key = conductor
                .keystore()
                .clone()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .expect("Failed to generate agent key");
            self.i += 1;
            if self.i >= len {
                self.i = 0;
            }
            self.total += 1;
            let agents = vec![agent_key];
            let _apps = conductor
                .setup_app_for_agents("app", &agents, &[self.test_dna.clone()])
                .await
                .unwrap();
        }
        println!("{}:{:?}", self.total, start.elapsed());
    }

    async fn run_inline_zome(&mut self) {
        use holochain_keystore::KeystoreSenderExt;
        if self.total > 300 {
            return;
        }
        let start = std::time::Instant::now();
        for _ in 0..1 {
            let len = self.conductors.len();
            let conductor = self.conductors.get_mut(self.i).unwrap();
            let agent_key = conductor
                .keystore()
                .clone()
                .generate_sign_keypair_from_pure_entropy()
                .await
                .expect("Failed to generate agent key");
            self.i += 1;
            if self.i >= len {
                self.i = 0;
            }
            self.total += 1;
            let agents = vec![agent_key];
            let _apps = conductor
                .setup_app_for_agents("app", &agents, &[self.inline_dna.clone()])
                .await
                .unwrap();
        }
        println!("{}:{:?}", self.total, start.elapsed());
    }

    async fn consistency(&mut self, client: &Client, num_machines: u64, url: &str) {
        sync(&client, num_machines, url).await;
        let num_peers = num(client, url).await;
        let mut peers = Vec::new();
        for c in &self.conductors {
            let info = c
                .get_agent_infos(None)
                .await
                .unwrap()
                .into_iter()
                .next()
                .unwrap();
            peers.push(c.get_p2p_env(info.space.clone()).await);
        }
        let peer_refs = peers.iter().collect::<Vec<_>>();
        let mut cells = Vec::new();
        holochain::test_utils::fixed_peer_consistency_envs_others(
            &peer_refs,
            num_peers as usize,
            2000,
            std::time::Duration::from_millis(500),
        )
        .await;
        for c in &self.conductors {
            let ids = c.list_cell_ids().await.expect("Failed to list cell ids");
            for id in ids {
                cells.push(c.get_cell_env(&id).await.unwrap());
            }
        }
        let cell_refs = cells.iter().collect::<Vec<_>>();
        // consistency_envs(&cell_refs, 2000, std::time::Duration::from_millis(500)).await;
        holochain::test_utils::fixed_consistency_envs_others(
            &cell_refs,
            num_peers as usize * 7,
            2000,
            std::time::Duration::from_millis(500),
        )
        .await;
        sync(&client, num_machines, url).await;
    }
}

async fn setup(num_conductors: usize, url: &str) -> Producers {
    let config = || {
        let tuning: KitsuneP2pTuningParams = KitsuneP2pTuningParams::default();
        // tuning.gossip_peer_on_success_next_gossip_delay_ms = 1000 * 10;
        // tuning.gossip_peer_on_error_next_gossip_delay_ms = 1000 * 20;

        let mut network = KitsuneP2pConfig::default();
        network.tuning_params = Arc::new(tuning);
        network.bootstrap_service = Some(url2::Url2::parse(url));
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
    for _ in 0..num_conductors {
        configs.push(config());
    }
    let conductors = SweetConductorBatch::from_configs(configs.clone()).await;

    let conductors = conductors.into_inner().into_iter();

    let (dna, _) = SweetDnaFile::unique_from_test_wasms(vec![
        holochain_wasm_test_utils::TestWasm::ValidateValid,
    ])
    .await
    .unwrap();

    let unit_entry_def = holochain_zome_types::EntryDef::default_with_id("unit");
    let inline_zome = holochain_zome_types::InlineZome::new_unique(vec![unit_entry_def.clone()])
        .callback(
            "validate",
            |_api, _data: holochain_zome_types::ValidateData| {
                Ok(holochain::core::ribosome::guest_callback::validate::ValidateResult::Valid)
            },
        );
    let (inline_dna, _) = SweetDnaFile::unique_from_inline_zome("app", inline_zome)
        .await
        .unwrap();
    Producers {
        conductors: conductors.collect(),
        i: 0,
        total: 0,
        test_dna: dna,
        inline_dna,
    }
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(24)
        .build()
        .unwrap()
}

async fn clear(client: &Client, url: &str) {
    let res = client
        .post(url)
        .header("X-Op", "clear")
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
}
async fn num(client: &Client, url: &str) -> u64 {
    let res = client
        .post(url)
        .header("X-Op", "num")
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
    let num_peers: u64 =
        kitsune_p2p_types::codec::rmp_decode(&mut res.bytes().await.unwrap().as_ref()).unwrap();
    println!("num_peers {}", num_peers);
    num_peers
}
async fn sync(client: &Client, num_wait_for: u64, url: &str) {
    let mut body_data = Vec::new();
    kitsune_p2p_types::codec::rmp_encode(&mut body_data, num_wait_for).unwrap();
    let res = client
        .post(url)
        .header("X-Op", "sync")
        .header(reqwest::header::CONTENT_TYPE, "application/octet")
        .body(body_data)
        .send()
        .await
        .unwrap();
    assert!(res.status().is_success());
}
