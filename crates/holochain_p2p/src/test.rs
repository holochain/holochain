use crate::actor::*;
use crate::HolochainP2pCell;
use crate::*;
use ::fixt::prelude::*;
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;

struct StubNetwork;

impl ghost_actor::GhostHandler<HolochainP2p> for StubNetwork {}
impl ghost_actor::GhostControlHandler for StubNetwork {}

#[allow(unused_variables)]
impl HolochainP2pHandler for StubNetwork {
    fn handle_join(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }
    fn handle_leave(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }
    fn handle_call_remote(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        to_agent: AgentPubKey,
        zome_name: ZomeName,
        fn_name: FunctionName,
        cap: Option<CapSecret>,
        payload: ExternIO,
    ) -> HolochainP2pHandlerResult<SerializedBytes> {
        Err("stub".into())
    }
    fn handle_publish(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        request_validation_receipt: bool,
        countersigning_session: bool,
        dht_hash: holo_hash::AnyDhtHash,
        ops: Vec<(holo_hash::DhtOpHash, holochain_types::dht_op::DhtOp)>,
        timeout_ms: Option<u64>,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }
    fn handle_get_validation_package(
        &mut self,
        input: actor::GetValidationPackage,
    ) -> HolochainP2pHandlerResult<ValidationPackageResponse> {
        Err("stub".into())
    }
    fn handle_get(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireOps>> {
        Err("stub".into())
    }
    fn handle_get_meta(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> HolochainP2pHandlerResult<Vec<MetadataSet>> {
        Err("stub".into())
    }
    fn handle_get_links(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireLinkOps>> {
        Err("stub".into())
    }
    fn handle_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> HolochainP2pHandlerResult<Vec<AgentActivityResponse<HeaderHash>>> {
        Err("stub".into())
    }
    fn handle_send_validation_receipt(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        from_agent: AgentPubKey,
        receipt: SerializedBytes,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }
    fn handle_new_integrated_data(&mut self, dna_hash: DnaHash) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }
    fn handle_authority_for_hash(
        &mut self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        dht_hash: AnyDhtHash,
    ) -> HolochainP2pHandlerResult<bool> {
        Err("stub".into())
    }
    fn handle_countersigning_authority_response(
        &mut self,
        dna_hash: DnaHash,
        from_agent: AgentPubKey,
        agents: Vec<AgentPubKey>,
        response: Vec<SignedHeader>,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }
}

/// Spawn a stub network that doesn't respond to any messages.
/// Use `test_network()` if you want a real test network.
pub async fn stub_network() -> ghost_actor::GhostSender<HolochainP2p> {
    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    let sender = channel_factory
        .create_channel::<HolochainP2p>()
        .await
        .unwrap();

    tokio::task::spawn(builder.spawn(StubNetwork));

    sender
}

fixturator!(
    HolochainP2pCell;
    curve Empty {
        tokio_helper::block_forever_on(async {
            let holochain_p2p = crate::test::stub_network().await;
            holochain_p2p.to_cell(
                DnaHashFixturator::new(Empty).next().unwrap(),
                AgentPubKeyFixturator::new(Empty).next().unwrap(),
            )
        })
    };
    curve Unpredictable {
        HolochainP2pCellFixturator::new(Empty).next().unwrap()
    };
    curve Predictable {
        HolochainP2pCellFixturator::new(Empty).next().unwrap()
    };
);

#[cfg(test)]
mod tests {
    use crate::mock_network::{GossipProtocol, HolochainP2pMockChannel};
    use crate::*;
    use ::fixt::prelude::*;
    use futures::future::FutureExt;
    use ghost_actor::GhostControlSender;

    use holochain_zome_types::ValidationStatus;
    use kitsune_p2p::agent_store::AgentInfoSigned;
    use kitsune_p2p::dependencies::kitsune_p2p_proxy::TlsConfig;
    use kitsune_p2p::dependencies::url2;
    use kitsune_p2p::dht_arc::{ArcInterval, DhtArc, PeerDensity};
    use kitsune_p2p::KitsuneP2pConfig;

    macro_rules! newhash {
        ($p:ident, $c:expr) => {
            holo_hash::$p::from_raw_36([$c as u8; HOLO_HASH_UNTYPED_LEN].to_vec())
        };
    }

    fn test_setup() -> (
        holo_hash::DnaHash,
        holo_hash::AgentPubKey,
        holo_hash::AgentPubKey,
        holo_hash::AgentPubKey,
    ) {
        observability::test_run().unwrap();
        (
            newhash!(DnaHash, 's'),
            newhash!(AgentPubKey, '1'),
            newhash!(AgentPubKey, '2'),
            newhash!(AgentPubKey, '3'),
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_remote_workflow() {
        let (dna, a1, a2, _) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::default(),
            TlsConfig::new_ephemeral().await.unwrap(),
            None,
        )
        .await
        .unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    CallRemote { respond, .. } => {
                        respond.r(Ok(
                            async move { Ok(UnsafeBytes::from(b"yada".to_vec()).into()) }
                                .boxed()
                                .into(),
                        ));
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        let res = p2p
            .call_remote(
                dna,
                a1,
                a2,
                "".into(),
                "".into(),
                None,
                ExternIO::encode(b"yippo").unwrap(),
            )
            .await
            .unwrap();
        let res: Vec<u8> = UnsafeBytes::from(res).into();

        assert_eq!(b"yada".to_vec(), res);

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_validation_receipt_workflow() {
        let (dna, a1, a2, _) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::default(),
            TlsConfig::new_ephemeral().await.unwrap(),
            None,
        )
        .await
        .unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    ValidationReceiptReceived {
                        respond, receipt, ..
                    } => {
                        let receipt: Vec<u8> = UnsafeBytes::from(receipt).into();
                        assert_eq!(b"receipt-test".to_vec(), receipt);
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        p2p.send_validation_receipt(
            dna,
            a2,
            a1,
            UnsafeBytes::from(b"receipt-test".to_vec()).into(),
        )
        .await
        .unwrap();

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_publish_workflow() {
        let (dna, a1, a2, a3) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::default(),
            TlsConfig::new_ephemeral().await.unwrap(),
            None,
        )
        .await
        .unwrap();

        let recv_count = Arc::new(std::sync::atomic::AtomicU8::new(0));

        let recv_count_clone = recv_count.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Publish { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                        recv_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    QueryAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();
        p2p.join(dna.clone(), a3.clone()).await.unwrap();

        let header_hash = holo_hash::AnyDhtHash::from_raw_36_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::AnyDht::Header,
        );

        // this will fail because we can't reach any remote nodes
        // but, it still published locally, so our test will work
        let _ = p2p
            .publish(dna, a1, true, false, header_hash, vec![], Some(200))
            .await;

        assert_eq!(3, recv_count.load(std::sync::atomic::Ordering::SeqCst));

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_workflow() {
        observability::test_run().ok();

        let (dna, a1, a2, _a3) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::default(),
            TlsConfig::new_ephemeral().await.unwrap(),
            None,
        )
        .await
        .unwrap();

        let test_1 = WireOps::Element(WireElementOps {
            header: Some(Judged::valid(SignedHeader(fixt!(Header), fixt!(Signature)))),
            deletes: vec![],
            updates: vec![],
            entry: None,
        });
        let test_2 = WireOps::Element(WireElementOps {
            header: Some(Judged::valid(SignedHeader(fixt!(Header), fixt!(Signature)))),
            deletes: vec![],
            updates: vec![],
            entry: None,
        });

        let mut respond_queue = vec![test_1.clone(), test_2.clone()];
        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Get { respond, .. } => {
                        let resp = if let Some(h) = respond_queue.pop() {
                            h
                        } else {
                            panic!("too many requests!")
                        };
                        tracing::info!("test - get respond");
                        respond.r(Ok(async move { Ok(resp) }.boxed().into()));
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    QueryAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryOpHashes { respond, .. } => {
                        respond.r(Ok(async move { Ok(None) }.boxed().into()));
                    }
                    evt => println!("unhandled: {:?}", evt),
                }
            }
        });

        tracing::info!("test - join1");
        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        tracing::info!("test - join2");
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        let hash = holo_hash::AnyDhtHash::from_raw_36_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::AnyDht::Header,
        );

        tracing::info!("test - get");
        let res = p2p
            .get(dna, a1, hash, actor::GetOptions::default())
            .await
            .unwrap();

        tracing::info!("test - check res");
        assert_eq!(2, res.len());

        for r in res {
            assert!(r == test_1 || r == test_2);
        }

        tracing::info!("test - end of test shutdown p2p");
        p2p.ghost_actor_shutdown().await.unwrap();
        tracing::info!("test - end of test await task end");
        r_task.await.unwrap();
        tracing::info!("test - end of test - final done.");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_links_workflow() {
        let (dna, a1, a2, _) = test_setup();

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::default(),
            TlsConfig::new_ephemeral().await.unwrap(),
            None,
        )
        .await
        .unwrap();

        let test_1 = WireLinkOps {
            creates: vec![WireCreateLink::condense(
                fixt!(CreateLink),
                fixt!(Signature),
                ValidationStatus::Valid,
            )],
            deletes: vec![WireDeleteLink::condense(
                fixt!(DeleteLink),
                fixt!(Signature),
                ValidationStatus::Valid,
            )],
        };

        let test_1_clone = test_1.clone();
        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                let test_1_clone = test_1_clone.clone();
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    GetLinks { respond, .. } => {
                        respond.r(Ok(async move { Ok(test_1_clone) }.boxed().into()));
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        let hash = holo_hash::EntryHash::from_raw_36_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::Entry,
        );
        let link_key = WireLinkKey {
            base: hash,
            zome_id: 0.into(),
            tag: None,
        };

        let res = p2p
            .get_links(dna, a1, link_key, actor::GetLinksOptions::default())
            .await
            .unwrap();

        assert_eq!(2, res.len());

        for r in res {
            assert_eq!(r, test_1);
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    async fn agent_info() -> AgentInfoSigned {
        use ::fixt::prelude::*;
        use kitsune_p2p::fixt::*;
        AgentInfoSigned::sign(
            // FIXME: This space shouldn't be random.
            Arc::new(fixt!(KitsuneSpace)),
            Arc::new(fixt!(KitsuneAgent)),
            u32::MAX / 2,
            vec![url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/localhost/p/5778/-").into()],
            0,
                    std::time::UNIX_EPOCH.elapsed().unwrap().as_millis() as u64 + 60_000_000,
            |_| async move { Ok(Arc::new(fixt!(KitsuneSignature, Predictable))) },
        )
        .await
        .unwrap()
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_mock_get_workflow() {
        observability::test_run().ok();
        let info1 = agent_info().await;
        let info2 = agent_info().await;
        let peer_data = vec![info1.clone(), info2.clone()];
        let from_agents = vec![
            holo_hash::AgentPubKey::from_kitsune(&info1.agent),
            holo_hash::AgentPubKey::from_kitsune(&info2.agent),
        ];

        let (dna, a1, a2, _a3) = test_setup();
        let (from_kitsune_tx, to_kitsune_rx, mut channel) =
            HolochainP2pMockChannel::channel(peer_data, 1000, Default::default());
        tokio::task::spawn(async move {
            while let Some((msg, _)) = channel.next().await {
                dbg!(&msg);
                let mock_network::AddressedHolochainP2pMockMsg { agent, msg } = msg;
                match msg {
                    mock_network::HolochainP2pMockMsg::Gossip {
                        dna,
                        module,
                        gossip,
                    } => {
                        if let kitsune_p2p::GossipModuleType::ShardedRecent = module {
                            if let GossipProtocol::Sharded(gossip) = gossip {
                                use kitsune_p2p::gossip::sharded_gossip::*;
                                dbg!(&gossip);
                                match gossip {
                                    ShardedGossipWire::Initiate { .. } => {
                                        let msg = mock_network::HolochainP2pMockMsg::Gossip {
                                            dna: dna.clone(),
                                            module: module.clone(),
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::accept(vec![ArcInterval::Full]),
                                            ),
                                        };
                                        channel.send(msg.addressed(agent.clone())).await;
                                        let msg = mock_network::HolochainP2pMockMsg::Gossip {
                                            dna,
                                            module,
                                            gossip: GossipProtocol::Sharded(
                                                ShardedGossipWire::initiate(
                                                    vec![ArcInterval::Full],
                                                    100,
                                                ),
                                            ),
                                        };
                                        let from_agent =
                                            from_agents.iter().find(|a| **a != agent).unwrap();
                                        channel.send(msg.addressed(from_agent.clone())).await;
                                    }
                                    _ => (),
                                }
                            }
                        }
                    }
                    _ => (),
                }
            }
            dbg!();
        });

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::default(),
            TlsConfig::new_ephemeral().await.unwrap(),
            Some(Arc::new(
                kitsune_p2p::test_util::mock_network::mock_network(from_kitsune_tx, to_kitsune_rx),
            )),
        )
        .await
        .unwrap();

        let test_1 = WireOps::Element(WireElementOps {
            header: Some(Judged::valid(SignedHeader(fixt!(Header), fixt!(Signature)))),
            deletes: vec![],
            updates: vec![],
            entry: None,
        });
        let test_2 = WireOps::Element(WireElementOps {
            header: Some(Judged::valid(SignedHeader(fixt!(Header), fixt!(Signature)))),
            deletes: vec![],
            updates: vec![],
            entry: None,
        });

        let mut respond_queue = vec![test_1.clone(), test_2.clone()];
        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                let info1 = info1.clone();
                let info2 = info2.clone();
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Get { respond, .. } => {
                        let resp = if let Some(h) = respond_queue.pop() {
                            h
                        } else {
                            panic!("too many requests!")
                        };
                        tracing::info!("test - get respond");
                        respond.r(Ok(async move { Ok(resp) }.boxed().into()));
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    QueryAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![info1.clone(), info2.clone()]) }
                            .boxed()
                            .into()));
                    }
                    QueryGossipAgents { respond, .. } => {
                        let info1 = info1.clone();
                        let info2 = info2.clone();
                        respond.r(Ok(async move { Ok(vec![info1.clone(), info2.clone()]) }
                            .boxed()
                            .into()));
                    }
                    QueryOpHashes { respond, .. } => {
                        respond.r(Ok(async move { Ok(None) }.boxed().into()));
                    }
                    QueryPeerDensity { respond, .. } => {
                        let d = PeerDensity::new(DhtArc::full(0), 1.0, 2);
                        respond.r(Ok(async move { Ok(d) }.boxed().into()));
                    }
                    QueryAgentInfoSignedNearBasis { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![info1.clone(), info2.clone()]) }
                            .boxed()
                            .into()));
                    }
                    FetchOpData { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    evt => println!("unhandled: {:?}", evt),
                }
            }
        });

        tracing::info!("test - join1");
        p2p.join(dna.clone(), a1.clone()).await.unwrap();
        tracing::info!("test - join2");
        p2p.join(dna.clone(), a2.clone()).await.unwrap();

        let hash = holo_hash::AnyDhtHash::from_raw_36_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::AnyDht::Header,
        );

        tracing::info!("test - get");
        let res = p2p
            .get(dna, a1, hash, actor::GetOptions::default())
            .await
            .unwrap();

        tracing::info!("test - check res");
        assert_eq!(2, res.len());

        for r in res {
            assert!(r == test_1 || r == test_2);
        }

        tracing::info!("test - end of test shutdown p2p");
        p2p.ghost_actor_shutdown().await.unwrap();
        tracing::info!("test - end of test await task end");
        r_task.await.unwrap();
        tracing::info!("test - end of test - final done.");
    }
}
