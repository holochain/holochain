//! Tests for local and remote signals using rendezvous config
//!

use futures::StreamExt;
use hdk::prelude::ExternIO;
use holochain::sweettest::{SweetCell, SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
use holochain_types::signal::Signal;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::RemoteSignal;
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
#[cfg(feature = "slow_tests")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn remote_signals_batch() -> anyhow::Result<()> {
    holochain_trace::test_run().ok();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(2, SweetConductorConfig::rendezvous()).await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let app_batch = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let ((alice,), (bob,)): ((SweetCell,), (SweetCell,)) = app_batch.into_tuples();

    // Make sure the conductors are talking to each other before sending signals.
    conductors
        .require_initial_gossip_activity_for_cell(&bob, Duration::from_secs(90))
        .await;

    // Listen for signals on Bon's conductor. These are all the signals on that conductor but Bob has the only app on
    // that conductor for this test.
    let mut conductor_1_signal_stream = conductors[1].signal_stream().await.map(to_signal_message);

    // Call `signal_others` multiple times as Alice to send signals to Bob.
    for i in 0..6 {
        let _: () = conductors[0]
            .call(
                &alice.zome(TestWasm::EmitSignal),
                "signal_others",
                RemoteSignal {
                    agents: vec![bob.agent_pubkey().clone()],
                    signal: ExternIO::encode(SignalMessage {
                        value: format!("message {}", i),
                    })
                    .unwrap(),
                },
            )
            .await;
    }

    // Check that Bob receives all the signals.
    tokio::time::timeout(Duration::from_secs(60), async move {
        for i in 0..6 {
            let message = conductor_1_signal_stream.next().await.unwrap();
            assert_eq!(format!("message {}", i), message.value);
        }
    })
    .await
    .unwrap();

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "slow_tests")]
#[cfg_attr(target_os = "macos", ignore = "flaky")]
async fn remote_signals_broadcast() -> anyhow::Result<()> {
    holochain_trace::test_run().ok();

    let mut conductors =
        SweetConductorBatch::from_config_rendezvous(3, SweetConductorConfig::rendezvous()).await;

    let dna_file = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::EmitSignal])
        .await
        .0;

    let app_batch = conductors.setup_app("app", &[dna_file]).await.unwrap();

    let ((alice,), (bob,), (carol,)): ((SweetCell,), (SweetCell,), (SweetCell,)) =
        app_batch.into_tuples();

    // Make sure the conductors are talking to each other before sending signals.
    conductors
        .require_initial_gossip_activity_for_cell(&bob, Duration::from_secs(90))
        .await;

    // Listen for signals on Bob and Carol's conductors.
    let mut conductor_1_signal_stream = conductors[1].signal_stream().await.map(to_signal_message);
    let mut conductor_2_signal_stream = conductors[2].signal_stream().await.map(to_signal_message);

    // Call `signal_others` multiple times as Alice to send signals to Bob.
    let _: () = conductors[0]
        .call(
            &alice.zome(TestWasm::EmitSignal),
            "signal_others",
            RemoteSignal {
                agents: vec![bob.agent_pubkey().clone(), carol.agent_pubkey().clone()],
                signal: ExternIO::encode(SignalMessage {
                    value: "message from alice".to_string(),
                })
                .unwrap(),
            },
        )
        .await;

    // Check that Bob and Carol receive their signals.
    tokio::time::timeout(Duration::from_secs(60), async move {
        let bob_message = conductor_1_signal_stream.next().await.unwrap();
        assert_eq!("message from alice", bob_message.value);
        let carol_message = conductor_2_signal_stream.next().await.unwrap();
        assert_eq!("message from alice", carol_message.value);
    })
    .await
    .unwrap();

    Ok(())
}
