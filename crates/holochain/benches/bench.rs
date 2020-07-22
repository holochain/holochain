use ::fixt::prelude::*;
use criterion::BenchmarkId;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main, Criterion};
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain::core::ribosome::RibosomeT;
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_types::fixt::CapSecretFixturator;
use holochain_wasm_test_utils::TestWasm;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::HostInput;

pub fn wasm_call_n(c: &mut Criterion) {
    // tokio::runtime::Builder::new()
    // .enable_all()
    // .threaded_scheduler()
    // .core_threads(1)
    // .build()
    // .unwrap()
    // .spawn(async {

    let mut group = c.benchmark_group("wasm_call_n");

    // let ribosome =
    //     holochain::fixt::WasmRibosomeFixturator::new(holochain::fixt::curve::Zomes(vec![
    //         TestWasm::Bench.into(),
    //     ]))
    //     .next()
    //     .unwrap();

    let mut ribosome_fixturator =
        holochain::fixt::WasmRibosomeFixturator::new(holochain::fixt::curve::Zomes(vec![
            TestWasm::Bench.into(),
        ]));
    let mut cap_secret_fixturator = CapSecretFixturator::new(Unpredictable);
    let mut host_access_fixturator =
        holochain::fixt::ZomeCallHostAccessFixturator::new(fixt::Unpredictable);
    let mut cell_id_fixturator = holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable);
    let mut agent_key_fixturator = AgentPubKeyFixturator::new(Unpredictable);

    for n in vec![
        // 1 byte
        1,
        // 1 kb
        1_000,
        // 1 mb
        1_000_000,
        // 1 gb
        1_000_000_000,
    ] {
        println!("{}", n);
        group.throughput(Throughput::Bytes(n as _));
        let bytes = test_wasm_common::TestBytes::from(vec![0; n]);
        let sb: SerializedBytes = bytes.try_into().unwrap();

        group.bench_function(
            BenchmarkId::from_parameter(n),
            |b| {
                b.iter(|| {
                    // tokio::runtime::Builder::new()
                    // .enable_all()
                    // .threaded_scheduler()
                    // .core_threads(1)
                    // .build()
                    // .unwrap()
                    // .spawn(async {
                    let ca = host_access_fixturator.next().unwrap();
                    let r = ribosome_fixturator.next().unwrap();
                    let i = ZomeCallInvocation {
                        cell_id: cell_id_fixturator.next().unwrap(),
                        zome_name: TestWasm::Bench.into(),
                        cap: cap_secret_fixturator.next().unwrap(),
                        fn_name: "echo_bytes".into(),
                        payload: HostInput::new(sb.clone()),
                        provenance: agent_key_fixturator.next().unwrap(),
                    };
                    println!("{}", n);
                    r.maybe_call(ca.into(), &i, &TestWasm::Bench.into(), "echo_bytes".into())
                        .unwrap();
                });
                // });
            },
            // criterion::BatchSize::PerIteration,
        );
    }
    //     b.iter_batched(|| {
    //         (
    //             workspace_fixturator.next().unwrap(),
    //             ribosome_fixturator.next().unwrap(),
    //             ZomeCallInvocation {
    //                 cell_id: cell_id_fixturator.next().unwrap(),
    //                 zome_name: TestWasm::Bench.into(),
    //                 cap: cap_secret_fixturator.next().unwrap(),
    //                 fn_name: "echo_bytes".into(),
    //                 payload: HostInput::new(sb.clone()),
    //                 provenance: agent_key_fixturator.next().unwrap(),
    //             }
    //         )
    //     },
    //     |(w, r, i)| {
    //         println!("{}", n);
    //         r.call_zome_function(w, i).unwrap();
    //     },
    //     criterion::BatchSize::PerIteration,
    // );
    // }

    group.finish();

    // });
}

criterion_group!(benches, wasm_call_n,);

criterion_main!(benches);
