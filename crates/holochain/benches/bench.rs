use ::fixt::prelude::*;
use criterion::BenchmarkId;
use criterion::Throughput;
use criterion::BatchSize;
use criterion::{criterion_group, criterion_main, Criterion};
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain::core::ribosome::RibosomeT;
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_types::fixt::CapSecretFixturator;
use holochain_wasm_test_utils::TestWasm;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::HostInput;
use once_cell::sync::Lazy;

// let's register a lazy static tokio runtime
static TOKIO: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new()
        .threaded_scheduler()
        .build()
        .unwrap()
});

pub fn wasm_call_n(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_call_n");

    for n in vec![
        // 1 byte
        // 1,     // 1 kb
        1_000, // 1 mb
              // 1_000_000,
    ] {
        group.throughput(Throughput::Bytes(n as _));

        println!("{}", n);

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            // bytes
            let bytes = test_wasm_common::TestBytes::from(vec![0; n]);
            let sb: SerializedBytes = bytes.try_into().unwrap();

            let mut ribosome_fixturator =
                holochain::fixt::WasmRibosomeFixturator::new(holochain::fixt::curve::Zomes(vec![
                    TestWasm::Bench.into(),
                ]));
            let mut cap_secret_fixturator = CapSecretFixturator::new(Unpredictable);
            let mut host_access_fixturator =
                holochain::fixt::ZomeCallHostAccessFixturator::new(fixt::Unpredictable);
            let mut cell_id_fixturator =
                holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable);
            let mut agent_key_fixturator = AgentPubKeyFixturator::new(Unpredictable);

            TOKIO.enter(move || {
                b.iter_batched(
                    || {
                        let r = ribosome_fixturator.next().unwrap();
                        let ha = host_access_fixturator.next().unwrap();
                        let i = ZomeCallInvocation {
                            cell_id: cell_id_fixturator.next().unwrap(),
                            zome_name: TestWasm::Bench.into(),
                            cap: cap_secret_fixturator.next().unwrap(),
                            fn_name: "echo_bytes".into(),
                            payload: HostInput::new(sb.clone()),
                            provenance: agent_key_fixturator.next().unwrap(),
                        };
                        (r, ha, i)
                    },
                    |(r, ha, i)| {
                        r.maybe_call(ha.into(), &i, &TestWasm::Bench.into(), "echo_bytes".into())
                            .unwrap();
                    },
                    BatchSize::SmallInput,
                );
            });
        });
    }

    group.finish();

    // });
}

criterion_group!(benches, wasm_call_n,);

criterion_main!(benches);
