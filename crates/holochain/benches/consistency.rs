use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;

use holo_hash::EntryHash;
use holo_hash::EntryHashes;
use holochain::sweettest::*;
use holochain_test_wasm_common::AnchorInput;
use holochain_test_wasm_common::ManyAnchorInput;
use holochain_wasm_test_utils::TestWasm;
use tokio::runtime::Builder;
use tokio::runtime::Runtime;

// TODO: Produce a high data version of this bench.
// TODO: Add profile function to queries that need optimizing.
// TODO: Research indexing.

criterion_group!(benches, consistency);

criterion_main!(benches);

fn consistency(bench: &mut Criterion) {
    holochain_trace::test_run();
    let mut group = bench.benchmark_group("consistency");
    group.sample_size(
        std::env::var_os("BENCH_SAMPLE_SIZE")
            .and_then(|s| s.to_string_lossy().parse::<usize>().ok())
            .unwrap_or(100),
    );
    let runtime = rt();

    let (mut producer, mut consumer, others) = runtime.block_on(setup());
    if let Some(n) = std::env::var_os("BENCH_NUM_OPS") {
        let num_ops = n.to_string_lossy().parse::<usize>().unwrap();
        runtime.block_on(async {
            producer.fill(num_ops).await;
            let mut cells = vec![&consumer.cell, &producer.cell];
            cells.extend(others.cells.iter());
            await_consistency(50, cells).await.unwrap();
            // holochain_state::prelude::dump_tmp(consumer.cell.env());
        });
    }
    let mut cells = vec![consumer.cell.clone(), producer.cell.clone()];
    cells.extend(others.cells.clone());
    runtime.spawn(async move {
        producer.run().await;
        producer.conductor.shutdown().await;
    });
    group.bench_function(BenchmarkId::new("test", "test".to_string()), |b| {
        b.iter(|| {
            runtime.block_on(async { consumer.run(&cells[..]).await });
        });
    });
    runtime.block_on(async move {
        // The line below was added when migrating to rust edition 2021, per
        // https://doc.rust-lang.org/edition-guide/rust-2021/disjoint-capture-in-closures.html#migration
        let _ = &others;
        consumer.conductor.shutdown().await;
        drop(consumer);
        for mut c in others.conductors {
            c.shutdown().await;
            drop(c);
        }
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

struct Others {
    conductors: Vec<SweetConductor>,
    cells: Vec<SweetCell>,
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

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    async fn fill(&mut self, num_ops: usize) {
        let inputs: Vec<_> = (0..num_ops)
            .map(|i| AnchorInput("alice_fill".to_string(), i.to_string()))
            .collect();
        let _: Vec<EntryHash> = self
            .conductor
            .call(
                &self.cell.zome("anchor"),
                "anchor_many",
                ManyAnchorInput(inputs),
            )
            .await;
        // holochain_state::prelude::dump_tmp(self.cell.env());
    }
}

impl Consumer {
    async fn run(&mut self, cells: &[SweetCell]) {
        let start = std::time::Instant::now();
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
            if start.elapsed().as_secs() > 1 {
                for cell in cells {
                    await_consistency(1, [cell]).await.unwrap();
                }
            }
            // dump_tmp(self.cell.env());
            // dump_tmp(prod.env());
        }
        self.last = num;
        self.tx.send(num).await.unwrap();
    }
}

async fn setup() -> (Producer, Consumer, Others) {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Anchor]).await;
    let config =
        SweetConductorConfig::standard().tune_network_config(|nc| nc.disable_publish = true);
    let configs = vec![config; 5];
    let mut conductors = SweetConductorBatch::from_configs(configs.clone()).await;
    let apps = conductors.setup_app("app", [&dna]).await.unwrap();
    let mut cells = apps
        .into_inner()
        .into_iter()
        .map(|c| c.into_cells().into_iter().next().unwrap());
    let alice = cells.next().unwrap();
    let bobbo = cells.next().unwrap();

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
        Others {
            conductors: conductors.collect(),
            cells: cells.collect(),
        },
    )
}

pub fn rt() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}
