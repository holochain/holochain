#![allow(unused_imports, unused_variables)]

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use futures::StreamExt;
use holochain_types::prelude::fake_dna_zomes_named;
use holochain_types::prelude::write_fake_dna_file;
use holochain_util::tokio_helper;
use holochain_wasm_test_utils::TestWasm;
use holochain_websocket::local_websocket_client;
use std::time::Duration;
use tempfile::TempDir;

#[path = "../tests/test_utils/mod.rs"]
mod test_utils;

use test_utils::*;
use tracing::debug;

pub fn websocket_concurrent_install(c: &mut Criterion) {
    observability::test_run().ok();

    static REQ_TIMEOUT_MS: u64 = 15000;
    static NUM_DNA_CONCURRENCY: &[(u16, usize)] = &[(1, 1), (8, 4), (64, 10)];
    let admin_port = std::sync::atomic::AtomicUsize::new(9910);

    let mut group = c.benchmark_group("websocket");
    for (i, j) in NUM_DNA_CONCURRENCY {
        group.throughput(Throughput::Elements(*i as u64 * *j as u64));

        group.sample_size(10);
        group.measurement_time(Duration::from_secs(20));

        let bench_id = format!("{}_{}", i, j);
        let bench_fn = group.bench_function(BenchmarkId::from_parameter(bench_id.clone()), |b| {
            // separate the holochain spawn time from the measured time
            b.iter_batched(
                || {
                    tokio_helper::block_forever_on(async {
                        let admin_port =
                            admin_port.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u16;
                        let tmp_dir = tempfile::tempdir().unwrap();

                        let path = tmp_dir.path().to_path_buf();
                        let environment_path = path.clone();
                        let config = create_config(admin_port, environment_path);
                        let config_path = write_config(path, &config);
                        let holochain = start_holochain(config_path.clone()).await;

                        let (client, _) = local_websocket_client(admin_port).await.unwrap();

                        let zomes = vec![(TestWasm::Foo.into(), TestWasm::Foo.into())];

                        (client, holochain, zomes)
                    })
                },
                |(client, _holochain, zomes)| {
                    tokio_helper::block_forever_on(async move {
                        // without this holochain gets dropped too early
                        let _holochain = _holochain;

                        let install_tasks_stream =
                            futures::stream::iter((0..*i).into_iter().map(|g| {
                                let mut client = client.clone();
                                let zomes = zomes.clone();

                                tokio::spawn(async move {
                                    let agent_key =
                                        generate_agent_pubkey(&mut client, REQ_TIMEOUT_MS).await;
                                    debug!("[{}] Agent pub key generated: {}", g, agent_key);

                                    // Install Dna
                                    let name = format!("fake_dna_{}", g);
                                    let dna = fake_dna_zomes_named(
                                        &uuid::Uuid::new_v4().to_string(),
                                        &name,
                                        zomes,
                                    );

                                    let original_dna_hash = dna.dna_hash().clone();
                                    let (fake_dna_path, _tmpdir) =
                                        write_fake_dna_file(dna.clone()).await.unwrap();
                                    let dna_hash = register_and_install_dna_named(
                                        &mut client,
                                        original_dna_hash.clone(),
                                        agent_key,
                                        fake_dna_path.clone(),
                                        None,
                                        name.clone(),
                                        name.clone(),
                                        REQ_TIMEOUT_MS,
                                    )
                                    .await;

                                    debug!(
                                        "[{}] installed dna with hash {} and name {}",
                                        g, dna_hash, name
                                    );
                                })
                            }))
                            .buffer_unordered(*j);

                        let install_tasks =
                            futures::StreamExt::collect::<Vec<_>>(install_tasks_stream);

                        for r in install_tasks.await {
                            r.unwrap();
                        }
                    })
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(websocket, websocket_concurrent_install);
