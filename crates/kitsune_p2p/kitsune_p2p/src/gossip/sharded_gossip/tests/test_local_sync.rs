use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn local_sync_scenario() {
    let mut u = arbitrary::Unstructured::new(&NOISE);
    let evt_handler = MockKitsuneP2pEventHandler::new();
    let (evt_sender, _) = spawn_handler(evt_handler).await;
    let gossip = ShardedGossipLocal::test(GossipType::Recent, evt_sender, Default::default());

    let cert = Tx2Cert::arbitrary(&mut u);

    todo!("write scenario")
}
