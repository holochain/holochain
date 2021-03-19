use ::fixt::prelude::*;
use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use hdk::prelude::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holochain::core::ribosome::RibosomeT;
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static TOKIO_RUNTIME: Lazy<Mutex<tokio::runtime::Runtime>> = Lazy::new(|| {
    Mutex::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap(),
    )
});

static REAL_RIBOSOME: Lazy<Mutex<holochain::core::ribosome::real_ribosome::RealRibosome>> =
    Lazy::new(|| {
        Mutex::new(
            holochain::fixt::RealRibosomeFixturator::new(holochain::fixt::curve::Zomes(vec![
                TestWasm::Bench.into(),
            ]))
            .next()
            .unwrap(),
        )
    });

static CELL_ID: Lazy<Mutex<holochain_zome_types::cell::CellId>> = Lazy::new(|| {
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
            let bytes = vec![0; n];
            let _g = TOKIO_RUNTIME.lock().unwrap().enter();
            let ha = HOST_ACCESS_FIXTURATOR.lock().unwrap().next().unwrap();

            b.iter(|| {
                let zome: Zome = TestWasm::Bench.into();
                let i = ZomeCallInvocation {
                    cell_id: CELL_ID.lock().unwrap().clone(),
                    zome: zome.clone(),
                    cap: Some(CAP.lock().unwrap().clone()),
                    fn_name: "echo_bytes".into(),
                    payload: ExternIO::encode(&bytes).unwrap(),
                    provenance: AGENT_KEY.lock().unwrap().clone(),
                };
                REAL_RIBOSOME
                    .lock()
                    .unwrap()
                    .clone()
                    .maybe_call(ha.clone().into(), &i, &zome, &i.fn_name)
                    .unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, wasm_call_n,);

criterion_main!(benches);
