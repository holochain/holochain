use ::fixt::prelude::*;
use criterion::BenchmarkId;
use criterion::Throughput;
use criterion::{criterion_group, criterion_main, Criterion};
use hdk3::prelude::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain::core::ribosome::RibosomeT;
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_types::{dna::zome::Zome, fixt::CapSecretFixturator};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::ExternInput;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static TOKIO_RUNTIME: Lazy<Mutex<tokio::runtime::Runtime>> = Lazy::new(|| {
    Mutex::new(
        tokio::runtime::Builder::new()
            .threaded_scheduler()
            .enable_all()
            .build()
            .unwrap(),
    )
});

static WASM_RIBOSOME: Lazy<Mutex<holochain::core::ribosome::wasm_ribosome::WasmRibosome>> =
    Lazy::new(|| {
        Mutex::new(
            holochain::fixt::WasmRibosomeFixturator::new(holochain::fixt::curve::Zomes(vec![
                TestWasm::Bench.into(),
            ]))
            .next()
            .unwrap(),
        )
    });

static CELL_ID: Lazy<Mutex<holochain_types::cell::CellId>> = Lazy::new(|| {
    Mutex::new(
        holochain_types::fixt::CellIdFixturator::new(Unpredictable)
            .next()
            .unwrap(),
    )
});

static CAP: Lazy<Mutex<holochain_zome_types::capability::CapSecret>> =
    Lazy::new(|| Mutex::new(CapSecretFixturator::new(Unpredictable).next().unwrap()));

static AGENT_KEY: Lazy<Mutex<AgentPubKey>> =
    Lazy::new(|| Mutex::new(AgentPubKeyFixturator::new(Unpredictable).next().unwrap()));

static HOST_ACCESS_FIXTURATOR: Lazy<
    Mutex<holochain::fixt::ZomeCallHostAccessFixturator<Unpredictable>>,
> = Lazy::new(|| {
    Mutex::new(holochain::fixt::ZomeCallHostAccessFixturator::new(
        Unpredictable,
    ))
});

pub fn wasm_call_n(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_call_n");

    for n in vec![
        1,         // 1 byte
        1_000,     // 1 kb
        1_000_000, // 1 mb
    ] {
        group.throughput(Throughput::Bytes(n as _));

        group.bench_function(BenchmarkId::from_parameter(n), |b| {
            // bytes
            let bytes = test_wasm_common::TestBytes::from(vec![0; n]);
            let sb: SerializedBytes = bytes.try_into().unwrap();

            TOKIO_RUNTIME.lock().unwrap().enter(move || {
                let ha = HOST_ACCESS_FIXTURATOR.lock().unwrap().next().unwrap();

                b.iter(|| {
                    let zome: Zome = TestWasm::Bench.into();
                    let i = ZomeCallInvocation {
                        cell_id: CELL_ID.lock().unwrap().clone(),
                        zome_name: zome.zome_name().clone(),
                        cap: Some(CAP.lock().unwrap().clone()),
                        fn_name: "echo_bytes".into(),
                        payload: ExternInput::new(sb.clone()),
                        provenance: AGENT_KEY.lock().unwrap().clone(),
                    };
                    WASM_RIBOSOME
                        .lock()
                        .unwrap()
                        .clone()
                        .maybe_call(ha.clone().into(), &i, &zome, &i.fn_name)
                        .unwrap();
                });
            });
        });
    }

    group.finish();
}

criterion_group!(benches, wasm_call_n,);

criterion_main!(benches);
