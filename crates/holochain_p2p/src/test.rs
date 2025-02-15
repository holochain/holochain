use crate::actor::*;
use crate::HolochainP2pDna;
use crate::*;
use ::fixt::prelude::*;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::AgentPubKey;
use holo_hash::DnaHash;
use holochain_nonce::Nonce256Bits;
use holochain_zome_types::fixt::ActionFixturator;
use kitsune2_api::DhtArc;
use kitsune_p2p::dht::Arq;
struct StubNetwork;

impl ghost_actor::GhostHandler<HolochainP2p> for StubNetwork {}
impl ghost_actor::GhostControlHandler for StubNetwork {}

#[allow(unused_variables)]
impl HolochainP2pHandler for StubNetwork {
    fn handle_join(
        &mut self,
        dna_hash: DnaHash,
        agent_pub_key: AgentPubKey,
        maybe_agent_info: Option<AgentInfoSigned>,
        initial_arq: Option<Arq>,
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
        to_agent: AgentPubKey,
        zome_call_params_serialized: ExternIO,
        signature: Signature,
    ) -> HolochainP2pHandlerResult<SerializedBytes> {
        Err("stub".into())
    }

    fn handle_send_remote_signal(
        &mut self,
        dna_hash: DnaHash,
        to_agent_list: Vec<(AgentPubKey, ExternIO, Signature)>,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }

    fn handle_publish(
        &mut self,
        dna_hash: DnaHash,
        request_validation_receipt: bool,
        countersigning_session: bool,
        basis_hash: holo_hash::OpBasis,
        source: AgentPubKey,
        op_hash_list: Vec<OpHashSized>,
        timeout_ms: Option<u64>,
        reflect_ops: Option<Vec<DhtOp>>,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }

    fn handle_publish_countersign(
        &mut self,
        dna_hash: DnaHash,
        flag: bool,
        basis_hash: holo_hash::OpBasis,
        op: DhtOp,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }

    fn handle_get(
        &mut self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireOps>> {
        Err("stub".into())
    }

    fn handle_get_meta(
        &mut self,
        dna_hash: DnaHash,
        dht_hash: holo_hash::AnyDhtHash,
        options: actor::GetMetaOptions,
    ) -> HolochainP2pHandlerResult<Vec<MetadataSet>> {
        Err("stub".into())
    }

    fn handle_get_links(
        &mut self,
        dna_hash: DnaHash,
        link_key: WireLinkKey,
        options: actor::GetLinksOptions,
    ) -> HolochainP2pHandlerResult<Vec<WireLinkOps>> {
        Err("stub".into())
    }

    fn handle_count_links(
        &mut self,
        dna_hash: DnaHash,
        query: WireLinkQuery,
    ) -> HolochainP2pHandlerResult<CountLinksResponse> {
        Err("stub".into())
    }

    fn handle_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: actor::GetActivityOptions,
    ) -> HolochainP2pHandlerResult<Vec<AgentActivityResponse>> {
        Err("stub".into())
    }

    fn handle_must_get_agent_activity(
        &mut self,
        dna_hash: DnaHash,
        agent: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> HolochainP2pHandlerResult<Vec<MustGetAgentActivityResponse>> {
        Err("stub".into())
    }

    fn handle_send_validation_receipts(
        &mut self,
        dna_hash: DnaHash,
        to_agent: AgentPubKey,
        receipts: ValidationReceiptBundle,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }

    fn handle_new_integrated_data(&mut self, dna_hash: DnaHash) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }

    fn handle_authority_for_hash(
        &mut self,
        dna_hash: DnaHash,
        basis_hash: OpBasis,
    ) -> HolochainP2pHandlerResult<bool> {
        Err("stub".into())
    }
    fn handle_countersigning_session_negotiation(
        &mut self,
        dna_hash: DnaHash,
        agents: Vec<AgentPubKey>,
        message: event::CountersigningSessionNegotiationMessage,
    ) -> HolochainP2pHandlerResult<()> {
        Err("stub".into())
    }

    fn handle_dump_network_metrics(
        &mut self,
        dna_hash: Option<DnaHash>,
    ) -> HolochainP2pHandlerResult<String> {
        Err("stub".into())
    }

    fn handle_dump_network_stats(&mut self) -> HolochainP2pHandlerResult<String> {
        Err("stub".into())
    }

    fn handle_get_diagnostics(
        &mut self,
        dna_hash: DnaHash,
    ) -> HolochainP2pHandlerResult<kitsune_p2p::gossip::sharded_gossip::KitsuneDiagnostics> {
        Err("stub".into())
    }

    fn handle_storage_arcs(&mut self, dna_hash: DnaHash) -> HolochainP2pHandlerResult<Vec<DhtArc>> {
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
    HolochainP2pDna;
    curve Empty {
        tokio_helper::block_forever_on(async {
            let holochain_p2p = crate::test::stub_network().await;
            holochain_p2p.to_dna(
                DnaHashFixturator::new(Empty).next().unwrap(),
                None
            )
        })
    };
    curve Unpredictable {
        HolochainP2pDnaFixturator::new(Empty).next().unwrap()
    };
    curve Predictable {
        HolochainP2pDnaFixturator::new(Empty).next().unwrap()
    };
);

#[cfg(test)]
mod tests {
    use crate::HolochainP2pSender;
    use crate::*;
    use ::fixt::prelude::*;
    use futures::future::FutureExt;
    use ghost_actor::GhostControlSender;
    use holo_hash::fixt::AgentPubKeyFixturator;
    use holo_hash::fixt::DhtOpHashFixturator;
    use holochain_keystore::test_keystore;
    use holochain_types::prelude::*;
    use kitsune_p2p::dht::prelude::Topology;
    use kitsune_p2p::dht::{ArqStrat, PeerView, PeerViewQ};
    use kitsune_p2p::*;
    use kitsune_p2p_types::config::KitsuneP2pConfig;
    use kitsune_p2p_types::dependencies::lair_keystore_api::dependencies::sodoken::sign;
    use kitsune_p2p_types::tls::TlsConfig;
    use std::sync::Mutex;

    macro_rules! newhash {
        ($p:ident, $c:expr) => {
            holo_hash::$p::from_raw_36([$c; HOLO_HASH_UNTYPED_LEN].to_vec())
        };
    }

    fn test_setup() -> (
        holo_hash::DnaHash,
        holo_hash::AgentPubKey,
        holo_hash::AgentPubKey,
        holo_hash::AgentPubKey,
    ) {
        holochain_trace::test_run();
        (
            newhash!(DnaHash, b's'),
            fixt!(AgentPubKey, Predictable, 0),
            newhash!(AgentPubKey, b'2'),
            newhash!(AgentPubKey, b'3'),
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_remote_workflow() {
        let (dna, a1, a2, _) = test_setup();
        let keystore = test_keystore();
        let (signal_url, _signal_srv_handle) = kitsune_p2p::test_util::start_signal_srv().await;

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::from_signal_addr(signal_url),
            TlsConfig::new_ephemeral().await.unwrap(),
            kitsune_p2p::HostStub::new(),
            NetworkCompatParams::default(),
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
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryPeerDensity { respond, .. } => {
                        let view = test_peer_view();
                        respond.r(Ok(async move { Ok(view) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), a1.clone(), None, None).await.unwrap();
        p2p.join(dna.clone(), a2.clone(), None, None).await.unwrap();

        let zome_name: ZomeName = "".into();
        let fn_name: FunctionName = "".into();
        let nonce = Nonce256Bits::from([0; 32]);
        let cap_secret = None;
        let payload = ExternIO::encode(b"yippo").unwrap();
        let expires_at = (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap();

        let (bytes, bytes_hash) = ZomeCallParams {
            provenance: a1.clone(),
            cell_id: CellId::new(dna.clone(), a2.clone()),
            zome_name: zome_name.clone(),
            fn_name: fn_name.clone(),
            cap_secret,
            payload: payload.clone(),
            nonce,
            expires_at,
        }
        .serialize_and_hash()
        .unwrap();
        let signature = a1.sign_raw(&keystore, bytes_hash.into()).await.unwrap();

        let res = p2p
            .call_remote(dna, a2, ExternIO(bytes), signature)
            .await
            .unwrap();
        let res: Vec<u8> = UnsafeBytes::from(res).into();

        assert_eq!(b"yada".to_vec(), res);

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_call_remote_interface() {
        let (dna, alice, bob, _) = test_setup();
        let keystore = test_keystore();
        let (signal_url, _signal_srv_handle) = kitsune_p2p::test_util::start_signal_srv().await;

        let (p2p, mut evt) = spawn_holochain_p2p(
            KitsuneP2pConfig::from_signal_addr(signal_url),
            TlsConfig::new_ephemeral().await.unwrap(),
            kitsune_p2p::HostStub::new(),
            NetworkCompatParams::default(),
        )
        .await
        .unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    CallRemote { respond, .. } => {
                        println!("incoming call remote");
                        respond.r(Ok(
                            async move { Ok(UnsafeBytes::from(b"yada".to_vec()).into()) }
                                .boxed()
                                .into(),
                        ));
                    }
                    SignNetworkData { respond, .. } => {
                        println!("incoming sign network data");
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        println!("incoming put agent info signed");
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryPeerDensity { respond, .. } => {
                        println!("incoming query peer density");
                        let view = test_peer_view();
                        respond.r(Ok(async move { Ok(view) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), alice.clone(), None, None)
            .await
            .unwrap();
        p2p.join(dna.clone(), bob.clone(), None, None)
            .await
            .unwrap();

        let zome_name: ZomeName = "".into();
        let fn_name: FunctionName = "".into();
        let nonce = Nonce256Bits::from([0; 32]);
        let cap_secret = None;
        let payload = ExternIO::encode(b"yippo").unwrap();
        let expires_at = (Timestamp::now() + std::time::Duration::from_secs(10)).unwrap();

        let (bytes, bytes_hash) = ZomeCallParams {
            provenance: alice.clone(),
            cell_id: CellId::new(dna.clone(), bob.clone()),
            zome_name: zome_name.clone(),
            fn_name: fn_name.clone(),
            cap_secret,
            payload: payload.clone(),
            nonce,
            expires_at,
        }
        .serialize_and_hash()
        .unwrap();
        let signature = alice.sign_raw(&keystore, bytes_hash.into()).await.unwrap();

        let res = p2p
            .call_remote(dna, bob, ExternIO(bytes), signature)
            .await
            .unwrap();
        let res: Vec<u8> = UnsafeBytes::from(res).into();
        println!("received res {res:?}");

        assert_eq!(b"yada".to_vec(), res);

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_send_validation_receipt_workflow() {
        let (dna, a1, a2, _) = test_setup();
        let (signal_url, _signal_srv_handle) = kitsune_p2p::test_util::start_signal_srv().await;

        let (p2p, mut evt): (HolochainP2pRef, _) = spawn_holochain_p2p(
            KitsuneP2pConfig::from_signal_addr(signal_url),
            TlsConfig::new_ephemeral().await.unwrap(),
            kitsune_p2p::HostStub::new(),
            NetworkCompatParams::default(),
        )
        .await
        .unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    ValidationReceiptsReceived {
                        respond, receipts, ..
                    } => {
                        assert_eq!(1, receipts.into_iter().count());
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryPeerDensity { respond, .. } => {
                        let view = test_peer_view();
                        respond.r(Ok(async move { Ok(view) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), a1.clone(), None, None).await.unwrap();
        p2p.join(dna.clone(), a2.clone(), None, None).await.unwrap();

        let receipts = vec![SignedValidationReceipt {
            receipt: ValidationReceipt {
                dht_op_hash: fixt!(DhtOpHash),
                validation_status: ValidationStatus::Valid,
                validators: vec![],
                when_integrated: Timestamp::now(),
            },
            validators_signatures: vec![],
        }];
        p2p.send_validation_receipts(dna, a1, receipts.into())
            .await
            .unwrap();

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_publish_workflow() {
        let (dna, a1, a2, a3) = test_setup();
        let (signal_url, _signal_srv_handle) = kitsune_p2p::test_util::start_signal_srv().await;

        let mut tuning_params =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        tuning_params.gossip_strategy = "none".to_string();
        let tuning_params = Arc::new(tuning_params);
        let mut config = KitsuneP2pConfig::from_signal_addr(signal_url);
        config.tuning_params = tuning_params;

        let host_list = Arc::new(Mutex::new(Vec::new()));
        let test_host = {
            let host_list = host_list.clone();
            HostStub::with_check_op_data(Box::new(move |space, list, ctx| {
                host_list
                    .lock()
                    .unwrap()
                    .push(format!("{:?}:{:?}:{:?}", space, list, ctx,));
                async move { Ok(list.into_iter().map(|_| false).collect()) }
                    .boxed()
                    .into()
            }))
        };
        //let test_host = TestHost::default();

        let (p2p, mut evt) = spawn_holochain_p2p(
            config,
            TlsConfig::new_ephemeral().await.unwrap(),
            test_host,
            NetworkCompatParams::default(),
        )
        .await
        .unwrap();

        let r_task = tokio::task::spawn(async move {
            use tokio_stream::StreamExt;
            while let Some(evt) = evt.next().await {
                use crate::types::event::HolochainP2pEvent::*;
                match evt {
                    Publish { respond, .. } => {
                        respond.r(Ok(async move { Ok(()) }.boxed().into()));
                    }
                    SignNetworkData { respond, .. } => {
                        respond.r(Ok(async move { Ok([0; 64].into()) }.boxed().into()));
                    }
                    PutAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryAgentInfoSignedNearBasis { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryPeerDensity { respond, .. } => {
                        let view = test_peer_view();
                        respond.r(Ok(async move { Ok(view) }.boxed().into()));
                    }
                    oth => {
                        tracing::warn!(?oth, "@@@");
                    }
                }
            }
        });

        p2p.join(dna.clone(), a1.clone(), None, None).await.unwrap();
        p2p.join(dna.clone(), a2.clone(), None, None).await.unwrap();
        p2p.join(dna.clone(), a3.clone(), None, None).await.unwrap();

        let action_hash = holo_hash::OpBasis::from_raw_36_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::AnyLinkable::Action,
        );

        // this will fail because we can't reach any remote nodes
        // but, it still published locally, so our test will work
        let _ = p2p
            .publish(
                dna,
                true,
                false,
                action_hash,
                a1.clone(),
                vec![],
                Some(200),
                None,
            )
            .await;

        assert_eq!(
            "KitsuneSpace(0x737373737373737373737373737373737373737373737373737373737373737373737373):[]:Some(FetchContext(1))",
            host_list.lock().unwrap()[0],
        );

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_get_workflow() {
        holochain_trace::test_run();

        let (dna, a1, a2, _a3) = test_setup();
        let (signal_url, _signal_srv_handle) = kitsune_p2p::test_util::start_signal_srv().await;

        let cert = TlsConfig::new_ephemeral().await.unwrap();

        let mut params =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        params.default_rpc_multi_remote_agent_count = 1;
        params.default_rpc_multi_remote_request_grace_ms = 100;
        let mut config = KitsuneP2pConfig::from_signal_addr(signal_url);
        config.tuning_params = Arc::new(params);
        let (p2p, mut evt) = spawn_holochain_p2p(
            config,
            cert,
            kitsune_p2p::HostStub::new(),
            NetworkCompatParams::default(),
        )
        .await
        .unwrap();

        let test_1 = WireOps::Record(WireRecordOps {
            action: Some(Judged::valid(SignedAction::new(
                fixt!(Action),
                fixt!(Signature),
            ))),
            deletes: vec![],
            updates: vec![],
            entry: None,
        });
        let test_2 = WireOps::Record(WireRecordOps {
            action: Some(Judged::valid(SignedAction::new(
                fixt!(Action),
                fixt!(Signature),
            ))),
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
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryAgentInfoSigned { respond, .. } => {
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryOpHashes { respond, .. } => {
                        respond.r(Ok(async move { Ok(None) }.boxed().into()));
                    }
                    QueryPeerDensity { respond, .. } => {
                        let view = test_peer_view();
                        respond.r(Ok(async move { Ok(view) }.boxed().into()));
                    }
                    evt => tracing::trace!("unhandled: {:?}", evt),
                }
            }
        });

        tracing::info!("test - join1");
        p2p.join(dna.clone(), a1.clone(), None, None).await.unwrap();
        tracing::info!("test - join2");
        p2p.join(dna.clone(), a2.clone(), None, None).await.unwrap();

        let hash = holo_hash::AnyDhtHash::from_raw_36_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::AnyDht::Action,
        );

        tracing::info!("test - get");
        let res = p2p
            .get(dna, hash, crate::actor::GetOptions::default())
            .await
            .unwrap();

        tracing::info!("test - check res");
        assert_eq!(1, res.len());

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
        let (signal_url, _signal_srv_handle) = kitsune_p2p::test_util::start_signal_srv().await;

        let mut params =
            kitsune_p2p_types::config::tuning_params_struct::KitsuneP2pTuningParams::default();
        params.default_rpc_multi_remote_agent_count = 1;
        params.default_rpc_multi_remote_request_grace_ms = 100;
        let mut config = KitsuneP2pConfig::from_signal_addr(signal_url);
        config.tuning_params = Arc::new(params);

        let (p2p, mut evt) = spawn_holochain_p2p(
            config,
            TlsConfig::new_ephemeral().await.unwrap(),
            kitsune_p2p::HostStub::new(),
            NetworkCompatParams::default(),
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
                        respond.r(Ok(async move { Ok(vec![]) }.boxed().into()));
                    }
                    QueryPeerDensity { respond, .. } => {
                        let view = test_peer_view();
                        respond.r(Ok(async move { Ok(view) }.boxed().into()));
                    }
                    _ => {}
                }
            }
        });

        p2p.join(dna.clone(), a1.clone(), None, None).await.unwrap();
        p2p.join(dna.clone(), a2.clone(), None, None).await.unwrap();

        let hash = holo_hash::EntryHash::from_raw_36_and_type(
            b"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_vec(),
            holo_hash::hash_type::Entry,
        );
        let link_key = WireLinkKey {
            base: hash.into(),
            type_query: LinkTypeFilter::single_dep(0.into()),
            tag: None,
            after: None,
            before: None,
            author: None,
        };

        let res = p2p
            .get_links(dna, link_key, crate::actor::GetLinksOptions::default())
            .await
            .unwrap();

        assert_eq!(1, res.len());

        for r in res {
            assert_eq!(r, test_1);
        }

        p2p.ghost_actor_shutdown().await.unwrap();
        r_task.await.unwrap();
    }

    fn test_peer_view() -> PeerView {
        PeerViewQ::new(Topology::standard_epoch_full(), ArqStrat::default(), vec![]).into()
    }
}
