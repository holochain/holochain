use super::*;

use kitsune_p2p_types::tx2::tx2_adapter::test_utils::*;
use kitsune_p2p_types::tx2::tx2_adapter::*;

use once_cell::sync::Lazy;

use std::sync::atomic;

/// create a "signed" agent info uniq by a u8
fn make_agent(c: u8) -> AgentInfoSigned {
    let space = Arc::new(KitsuneSpace(vec![0; 36]));
    let agent = Arc::new(KitsuneAgent(vec![c; 36]));
    futures::executor::block_on(AgentInfoSigned::sign(
        space,
        agent,
        u32::MAX,
        vec![format!("fake://{}", c).into()],
        42,
        69,
        |_| async move { Ok(Arc::new(vec![0; 64].into())) },
    ))
    .unwrap()
}

static A1: Lazy<AgentInfoSigned> = Lazy::new(|| make_agent(1));
static A2: Lazy<AgentInfoSigned> = Lazy::new(|| make_agent(2));
static A3: Lazy<AgentInfoSigned> = Lazy::new(|| make_agent(3));
static A4: Lazy<AgentInfoSigned> = Lazy::new(|| make_agent(4));

static CERT_IDX: atomic::AtomicU8 = atomic::AtomicU8::new(0);

/// create incrementing "uniq" Tx2Certs
fn next_cert() -> Tx2Cert {
    vec![CERT_IDX.fetch_add(1, atomic::Ordering::Relaxed); 32].into()
}

/// spawn a ghost actor for SpaceInternal
async fn build_space_internal(
    m: MockSpaceInternalHandler,
) -> ghost_actor::GhostSender<SpaceInternal> {
    let b = ghost_actor::actor_builder::GhostActorBuilder::new();
    let i_s = b
        .channel_factory()
        .create_channel::<SpaceInternal>()
        .await
        .unwrap();
    tokio::task::spawn(b.spawn(m));
    i_s
}

/// spawn a ghost actor for KitsuneP2pEvent
async fn build_event_handler(
    m: MockKitsuneP2pEventHandler,
) -> futures::channel::mpsc::Sender<KitsuneP2pEvent> {
    let b = ghost_actor::actor_builder::GhostActorBuilder::new();
    let (evt_sender, r) = futures::channel::mpsc::channel::<KitsuneP2pEvent>(4096);
    b.channel_factory().attach_receiver(r).await.unwrap();
    tokio::task::spawn(b.spawn(m));
    evt_sender
}

/// spawn an endpoint adapter factory, then fetch a single ep handle
async fn build_ep_hnd(config: Arc<KitsuneP2pConfig>, m: MockBindAdapt) -> Tx2EpHnd<wire::Wire> {
    let f: AdapterFactory = Arc::new(m);
    let f = tx2_pool_promote(f, config.tuning_params.clone());
    let f = tx2_api::<wire::Wire>(f, Default::default());

    let mut ep = f
        .bind("fake://0", config.tuning_params.implicit_timeout())
        .await
        .unwrap();
    let ep_hnd = ep.handle().clone();
    tokio::task::spawn(async move { while let Some(_) = ep.next().await {} });
    ep_hnd
}

#[tokio::test]
async fn test_rpc_multi_logic_mocked() {
    observability::test_run().ok();

    // allow fake timing during test
    tokio::time::pause();

    let space = Arc::new(KitsuneSpace(vec![0; 36]));
    let this_addr = url2::url2!("fake://");

    // build our "SpaceInternal" sender
    let mut m = MockSpaceInternalHandler::new();
    // just make is_agent_local always return false
    m.expect_handle_is_agent_local()
        .returning(|_| Ok(async move { Ok(false) }.boxed().into()));
    let i_s = build_space_internal(m).await;

    // build our "KitsuneP2pEvent" sender
    let mut m = MockKitsuneP2pEventHandler::new();
    let start = tokio::time::Instant::now();
    // don't return any infos to start, then return 4 to test our loops
    m.expect_handle_query_agents().returning(move |_| {
        let mut out = Vec::new();
        if start.elapsed().as_secs_f64() > 1.0 {
            out.push(A1.clone());
            out.push(A2.clone());
            out.push(A3.clone());
            out.push(A4.clone());
        }
        Ok(async move { Ok(out) }.boxed().into())
    });
    let evt_sender = build_event_handler(m).await;

    let config = Arc::new(KitsuneP2pConfig::default());

    // mock out our bind adapter
    let mut m = MockBindAdapt::new();
    m.expect_bind().returning(move |_, _| {
        async move {
            let mut m = MockEndpointAdapt::new();
            let uniq = Uniq::default();
            // return a uniq identifier
            m.expect_uniq().returning(move || uniq);
            let cert = next_cert();
            // return a uniq cert
            m.expect_local_cert().returning(move || cert.clone());
            // allow making "outgoing" connections that will respond how
            // we configure them to
            m.expect_connect().returning(move |_, _| {
                async move {
                    let (w_send, w_recv) = tokio::sync::mpsc::channel(1);
                    let w_recv = Arc::new(parking_lot::Mutex::new(Some(w_recv)));

                    // mock out our connection adapter
                    let mut m = MockConAdapt::new();
                    let uniq = Uniq::default();
                    // return a uniq identifier
                    m.expect_uniq().returning(move || uniq);
                    // this is an "outgoing" connection
                    m.expect_dir().returning(|| Tx2ConDir::Outgoing);
                    let cert = next_cert();
                    // return a uniq cert to identify our peer
                    m.expect_peer_cert().returning(move || cert.clone());
                    // allow making "outgoing" channels that will respond
                    // how we configure them to
                    m.expect_out_chan().returning(move |_| {
                        let w_send = w_send.clone();
                        let mut m = MockAsFramedWriter::new();
                        // when we get an outgoing write event
                        // turn around and respond appropriately for
                        // our test
                        m.expect_write().returning(move |msg_id, mut buf, _| {
                            let w_send = w_send.clone();
                            use kitsune_p2p_types::codec::Codec;
                            let (_, wire) = wire::Wire::decode_ref(&buf).unwrap();
                            async move {
                                let resp = match wire {
                                    wire::Wire::Call(wire::Call {
                                        space: _,
                                        from_agent: _,
                                        to_agent: _,
                                        data,
                                    }) => {
                                        println!(
                                            "GOT CALL: {:?} {}",
                                            msg_id,
                                            String::from_utf8_lossy(&data)
                                        );
                                        wire::Wire::call_resp(data)
                                    }
                                    oth => {
                                        let reason =
                                            format!("test doesn't handle {:?} requests", oth);
                                        wire::Wire::failure(reason)
                                    }
                                };
                                let data = resp.encode_vec().unwrap();
                                buf.clear();
                                buf.extend_from_slice(&data);
                                // forward this message to our "recv" side
                                w_send.send((msg_id.as_res(), buf)).await.unwrap();

                                Ok(())
                            }
                            .boxed()
                        });
                        let out: OutChan = Box::new(m);
                        async move { Ok(out) }.boxed()
                    });
                    let con: Arc<dyn ConAdapt> = Arc::new(m);

                    // make an incoming reader that will forward responses
                    // according to the logic in the writer above
                    let mut m = MockAsFramedReader::new();
                    m.expect_read().returning(move |_| {
                        let w_recv = w_recv.clone();
                        async move {
                            let mut w = match w_recv.lock().take() {
                                Some(w) => w,
                                None => return Err("end".into()),
                            };

                            let r = match w.recv().await {
                                Some(r) => r,
                                None => return Err("end".into()),
                            };

                            *w_recv.lock() = Some(w);
                            Ok(r)
                        }
                        .boxed()
                    });

                    // we'll only establish one single in channel
                    let once: InChan = Box::new(m);
                    let once =
                        futures::stream::once(async move { async move { Ok(once) }.boxed() });
                    // then just pend the stream
                    let s = once.chain(futures::stream::pending());
                    let rcv = gen_mock_in_chan_recv_adapt(s.boxed());
                    Ok((con, rcv))
                }
                .boxed()
            });
            let ep: Arc<dyn EndpointAdapt> = Arc::new(m);

            // we will never receive any "incoming" connections
            let c = gen_mock_con_recv_adapt(futures::stream::pending().boxed());

            Ok((ep, c))
        }
        .boxed()
    });
    let ep_hnd = build_ep_hnd(config.clone(), m).await;

    // build up the ro_inner that discover calls expect
    let ro_inner = Arc::new(SpaceReadOnlyInner {
        space: space.clone(),
        this_addr,
        i_s,
        evt_sender,
        ep_hnd,
        parallel_notify_permit: Arc::new(tokio::sync::Semaphore::new(
            config.tuning_params.concurrent_limit_per_thread,
        )),
        config,
    });

    let agent = Arc::new(KitsuneAgent(vec![0; 36]));
    let basis = Arc::new(KitsuneBasis(vec![0; 36]));

    // excercise the rpc multi logic
    let res = handle_rpc_multi(
        actor::RpcMulti {
            space,
            from_agent: agent.clone(),
            basis,
            payload: b"test".to_vec(),
            max_remote_agent_count: 3,
            max_timeout: KitsuneTimeout::from_millis(30000),
            remote_request_grace_ms: 3000,
        },
        ro_inner,
        HashSet::new(),
    )
    .await
    .unwrap();

    // await responses
    println!("{:#?}", res);
    assert_eq!(3, res.len());
    for r in res {
        let RpcMultiResponse { response, .. } = r;
        assert_eq!(b"test", response.as_slice());
    }
}
