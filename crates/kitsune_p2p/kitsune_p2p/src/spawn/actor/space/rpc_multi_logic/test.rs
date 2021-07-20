use super::*;

use kitsune_p2p_types::tx2::tx2_adapter::test_utils::*;
use kitsune_p2p_types::tx2::tx2_adapter::*;

#[tokio::test]
async fn test_rpc_multi_logic_mocked() {
    let space = Arc::new(KitsuneSpace(vec![0; 36]));
    let this_addr = url2::url2!("fake://");

    let m = MockSpaceInternalHandler::new();
    let b = ghost_actor::actor_builder::GhostActorBuilder::new();
    let i_s = b
        .channel_factory()
        .create_channel::<SpaceInternal>()
        .await
        .unwrap();
    tokio::task::spawn(b.spawn(m));

    let m = MockKitsuneP2pEventHandler::new();
    let b = ghost_actor::actor_builder::GhostActorBuilder::new();
    let (evt_sender, r) = futures::channel::mpsc::channel::<KitsuneP2pEvent>(4096);
    b.channel_factory().attach_receiver(r).await.unwrap();
    tokio::task::spawn(b.spawn(m));

    let config = Arc::new(KitsuneP2pConfig::default());

    let mut m = MockBindAdapt::new();
    m.expect_bind().returning(|_, _| {
        async move {
            let mut m = MockEndpointAdapt::new();
            let uniq = Uniq::default();
            m.expect_uniq().returning(move || uniq);
            m.expect_local_cert().returning(|| vec![0; 32].into());
            let ep: Arc<dyn EndpointAdapt> = Arc::new(m);
            let c = gen_mock_con_recv_adapt(futures::stream::pending().boxed());
            Ok((ep, c))
        }
        .boxed()
    });
    let f: AdapterFactory = Arc::new(m);
    let f = tx2_pool_promote(f, config.tuning_params.clone());
    let f = tx2_api::<wire::Wire>(f, Default::default());

    let mut ep = f
        .bind("fake://", config.tuning_params.implicit_timeout())
        .await
        .unwrap();
    let ep_hnd = ep.handle().clone();
    tokio::task::spawn(async move { while let Some(_) = ep.next().await {} });

    let ro_inner = Arc::new(SpaceReadOnlyInner {
        space: space.clone(),
        this_addr,
        i_s,
        evt_sender,
        ep_hnd,
        config,
    });

    let agent = Arc::new(KitsuneAgent(vec![0; 36]));
    let basis = Arc::new(KitsuneBasis(vec![0; 36]));

    let res = handle_rpc_multi(
        actor::RpcMulti {
            space,
            from_agent: agent.clone(),
            basis,
            payload: b"test".to_vec(),
            max_remote_agent_count: 3,
            max_timeout: KitsuneTimeout::from_millis(3000),
            remote_request_grace_ms: 3000,
        },
        ro_inner,
        HashSet::new(),
    )
    .await
    .unwrap();

    println!("{:#?}", res);
}
