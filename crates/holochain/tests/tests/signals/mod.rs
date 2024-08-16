//! Tests for local and remote signals using rendezvous config
//!

use hdk::prelude::ExternIO;
use holochain::sweettest::*;
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Serialize, Deserialize, Debug)]
struct SignalMessage {
    value: String,
}

fn to_signal_message(signal: Signal) -> SignalMessage {
    match signal {
        Signal::App { signal, .. } => signal.into_inner().decode().unwrap(),
        _ => {
            panic!("Only expected app signals");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_signals_work_after_sbd_restart() {
    holochain_trace::test_run();

    const MAX: u64 = 30;

    let vous = SweetLocalRendezvous::new_raw().await;

    let vous_dyn: DynSweetRendezvous = vous.clone();
    let mut c1 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        vous_dyn.clone(),
    )
    .await;

    let mut c2 = SweetConductor::from_config_rendezvous(
        SweetConductorConfig::rendezvous(true),
        vous_dyn.clone(),
    )
    .await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let (app1,) = c1
        .setup_app("app", &[dna_file.clone()])
        .await
        .unwrap()
        .into_tuple();
    let (app2,) = c2.setup_app("app", &[dna_file]).await.unwrap().into_tuple();
    let a2 = app2.agent_pubkey().clone();

    let mut c2_rx = c2.subscribe_to_app_signals("app".to_string());

    let _: () = c1
        .call(
            &app1.zome(TestWasm::EmitSignal),
            "signal_others",
            RemoteSignal {
                agents: vec![a2.clone()],
                signal: ExternIO::encode(SignalMessage {
                    value: "hello".to_string(),
                })
                .unwrap(),
            },
        )
        .await;

    let msg = tokio::time::timeout(std::time::Duration::from_secs(MAX), c2_rx.recv())
        .await
        .unwrap()
        .map(to_signal_message)
        .unwrap()
        .value;

    assert_eq!("hello", &msg);

    // restart the signal (sbd) server so we get new ids
    vous.start_sig().await;

    // wait for that to propagate in holochain
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let done1 = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done2 = done1.clone();

    tokio::join!(
        async {
            let msg = tokio::time::timeout(std::time::Duration::from_secs(MAX), c2_rx.recv())
                .await
                .unwrap()
                .map(to_signal_message)
                .unwrap()
                .value;

            assert_eq!("world", &msg);

            done1.store(true, std::sync::atomic::Ordering::SeqCst);
        },
        async {
            for _ in 0..MAX {
                let _: () = c1
                    .call(
                        &app1.zome(TestWasm::EmitSignal),
                        "signal_others",
                        RemoteSignal {
                            agents: vec![a2.clone()],
                            signal: ExternIO::encode(SignalMessage {
                                value: "world".to_string(),
                            })
                            .unwrap(),
                        },
                    )
                    .await;

                if done2.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        },
    );
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
#[ignore = "flaky"]
async fn remote_signals_batch() -> anyhow::Result<()> {
    holochain_trace::test_run();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(3, SweetConductorConfig::rendezvous(true))
            .await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let app_batch = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let ((alice,), (bob,), (carol,)): ((SweetCell,), (SweetCell,), (SweetCell,)) =
        app_batch.into_tuples();

    // Make sure the conductors are talking to each other before sending signals.
    conductors[1]
        .require_initial_gossip_activity_for_cell(&bob, 3, Duration::from_secs(90))
        .await
        .unwrap();

    // Listen for signals on Bob's and Carol's conductors.
    // These are all the signals on that conductor but the only app installed
    // is the one for this test.
    let mut conductor_1_signal_rx = conductors[1].subscribe_to_app_signals("app".to_string());
    let mut conductor_2_signal_rx = conductors[2].subscribe_to_app_signals("app".to_string());

    // Call `signal_others` multiple times as Alice to send signals to Bob.
    for i in 0..6 {
        let _: () = conductors[0]
            .call(
                &alice.zome(TestWasm::EmitSignal),
                "signal_others",
                RemoteSignal {
                    agents: vec![bob.agent_pubkey().clone(), carol.agent_pubkey().clone()],
                    signal: ExternIO::encode(SignalMessage {
                        value: format!("message {}", i),
                    })
                    .unwrap(),
                },
            )
            .await;
    }

    // Check that Bob and Carol receive all the signals.
    tokio::time::timeout(Duration::from_secs(60), async move {
        for i in 0..6 {
            let msg_1 = conductor_1_signal_rx
                .recv()
                .await
                .map(to_signal_message)
                .unwrap()
                .value;
            let msg_2 = conductor_2_signal_rx
                .recv()
                .await
                .map(to_signal_message)
                .unwrap()
                .value;

            assert_eq!(msg_1, format!("message {}", i));
            assert_eq!(msg_1, msg_2);
        }
    })
    .await
    .unwrap();

    Ok(())
}
