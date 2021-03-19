use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use holo_hash::EntryHash;
use holo_hash::EntryHashes;
use holochain::test_utils::sweetest::*;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_conductor_api::AdminInterfaceConfig;
use holochain_conductor_api::InterfaceDriver;
use holochain_test_wasm_common::AnchorInput;
use holochain_wasm_test_utils::TestWasm;
use kitsune_p2p::KitsuneP2pConfig;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

criterion_group!(benches, consistency);

criterion_main!(benches);

fn consistency(bench: &mut Criterion) {
    let mut group = bench.benchmark_group("consistency");
    group.sample_size(20);
    let runtime = rt();

    let (mut producer, mut consumer) = runtime.block_on(setup());
    runtime.spawn(async move {
        producer.run().await;
        producer.conductor.shutdown_and_wait().await;
    });
    group.bench_function(BenchmarkId::new("test", format!("test")), |b| {
        b.iter(|| {
            runtime.block_on(async { consumer.run().await });
        });
    });
    runtime.block_on(async move {
        consumer.conductor.shutdown_and_wait().await;
        drop(consumer);
    });
    runtime.shutdown_background();
}

struct Producer {
    conductor: SweetConductor,
    cell: SweetCell,
    rx: tokio::sync::mpsc::Receiver<usize>,
}

struct Consumer {
    conductor: SweetConductor,
    cell: SweetCell,
    last: usize,
    tx: tokio::sync::mpsc::Sender<usize>,
}

impl Producer {
    async fn run(&mut self) {
        while let Some(mut i) = self.rx.recv().await {
            i += 1;
            let _: EntryHash = self
                .conductor
                .call(
                    &self.cell.zome("anchor"),
                    "anchor",
                    AnchorInput("alice".to_string(), i.to_string()),
                )
                .await;
        }
    }
}

impl Consumer {
    async fn run(&mut self) {
        let mut num = self.last;
        while num <= self.last {
            let hashes: EntryHashes = self
                .conductor
                .call(
                    &self.cell.zome("anchor"),
                    "list_anchor_addresses",
                    "alice".to_string(),
                )
                .await;
            num = hashes.0.len();
        }
        self.last = num;
        self.tx.send(num).await.unwrap();
    }
}

async fn setup() -> (Producer, Consumer) {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    let (dna, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Anchor])
        .await
        .unwrap();
    let config = || {
        let mut network = KitsuneP2pConfig::default();
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
    let configs = vec![config(), config()];
    let mut conductors = SweetConductorBatch::from_configs(configs.clone()).await;
    let apps = conductors.setup_app("app", &[dna]).await;
    let ((alice,), (bobbo,)) = apps.into_tuples();
    conductors.exchange_peer_info().await;
    let mut conductors = conductors.into_inner().into_iter();
    tx.send(0).await.unwrap();

    (
        Producer {
            conductor: conductors.next().unwrap(),
            cell: alice,
            rx,
        },
        Consumer {
            conductor: conductors.next().unwrap(),
            cell: bobbo,
            tx,
            last: 0,
        },
    )
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
