use ::fixt::prelude::*;
use criterion::BenchmarkId;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main, Criterion};
use hdk3::prelude::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain::core::ribosome::RibosomeT;
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_types::fixt::CapSecretFixturator;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::HostInput;

pub fn wasm_call_n(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_call_n");

    for n in vec![
        // 1 byte
        1,     // 1 kb
        1_000, // 1 mb
        1_000_000,
    ] {
        group.throughput(Throughput::Bytes(n as _));

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            // bytes
            let bytes = test_wasm_common::TestBytes::from(vec![0; n]);
            let sb: SerializedBytes = bytes.try_into().unwrap();

            let mut host_access_fixturator =
                holochain::fixt::ZomeCallHostAccessFixturator::new(Unpredictable);

            tokio::runtime::Builder::new()
                .threaded_scheduler()
                .build()
                .unwrap()
                .enter(move || {
                    let ribosome = holochain::fixt::WasmRibosomeFixturator::new(
                        holochain::fixt::curve::Zomes(vec![TestWasm::Bench.into()]),
                    )
                    .next()
                    .unwrap();
                    let cell_id = holochain_types::fixt::CellIdFixturator::new(Unpredictable)
                        .next()
                        .unwrap();
                    let cap = CapSecretFixturator::new(Unpredictable).next().unwrap();
                    let ha = host_access_fixturator.next().unwrap();
                    let agent_key = AgentPubKeyFixturator::new(Unpredictable).next().unwrap();
                    b.iter(|| {
                        let i = ZomeCallInvocation {
                            cell_id: cell_id.clone(),
                            zome_name: TestWasm::Bench.into(),
                            cap: cap.clone(),
                            fn_name: "echo_bytes".into(),
                            payload: HostInput::new(sb.clone()),
                            provenance: agent_key.clone(),
                        };
                        ribosome
                            .clone()
                            .maybe_call(
                                ha.clone().into(),
                                &i,
                                &i.zome_name.clone(),
                                i.fn_name.clone(),
                            )
                            .unwrap();
                    });
                });
        });
    }

    group.finish();
}

criterion_group!(benches, wasm_call_n,);

criterion_main!(benches);
