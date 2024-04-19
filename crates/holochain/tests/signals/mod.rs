//! Tests for local and remote signals using rendezvous config
//!

use futures::StreamExt;
use hdk::prelude::ExternIO;
use holochain::sweettest::{SweetCell, SweetConductorBatch, SweetConductorConfig, SweetDnaFile};
use holochain_types::prelude::*;
use holochain_wasm_test_utils::TestWasm;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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
    conductors
        .require_initial_gossip_activity_for_cell(&bob, Duration::from_secs(90))
        .await;

    // Listen for signals on Bob's and Carol's conductors.
    // These are all the signals on that conductor but the only app installed
    // is the one for this test.
    let conductor_1_signal_stream = conductors[1].signal_stream().await.map(to_signal_message);
    let conductor_2_signal_stream = conductors[2].signal_stream().await.map(to_signal_message);

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
        let msgs_1: HashSet<String> = conductor_1_signal_stream
            .take(6)
            .map(|m| m.value)
            .collect()
            .await;
        let msgs_2: HashSet<String> = conductor_2_signal_stream
            .take(6)
            .map(|m| m.value)
            .collect()
            .await;
        let expected = (0..6)
            .map(|i| format!("message {}", i))
            .collect::<HashSet<_>>();

        assert_eq!(msgs_1, expected);
        assert_eq!(msgs_1, msgs_2);
    })
    .await
    .unwrap();

    Ok(())
}
