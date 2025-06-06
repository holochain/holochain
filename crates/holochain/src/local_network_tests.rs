use crate::sweettest::*;
use futures::StreamExt;
use holo_hash::ActionHash;
use holochain_wasm_test_utils::TestWasm;
use test_case::test_case;

#[test_case(2)]
#[test_case(4)]
#[tokio::test(flavor = "multi_thread")]
async fn conductors_call_remote(num_conductors: usize) {
    holochain_trace::test_run();

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;

    let config = SweetConductorConfig::rendezvous(true);

    let mut conductors = SweetConductorBatch::from_config_rendezvous(num_conductors, config).await;

    let apps = conductors.setup_app("app", [&dna]).await.unwrap();
    let cells: Vec<_> = apps
        .into_inner()
        .into_iter()
        .map(|c| c.into_cells().into_iter().next().unwrap())
        .collect();

    // Make sure the conductors are talking to each other before we start making remote calls.
    for i in 0..num_conductors {
        conductors[i]
            .require_initial_gossip_activity_for_cell(
                &cells[i],
                num_conductors as u32 - 1,
                std::time::Duration::from_secs(60),
            )
            .await
            .unwrap();
    }

    let agents: Vec<_> = cells.iter().map(|c| c.agent_pubkey().clone()).collect();

    let iter = cells
        .clone()
        .into_iter()
        .zip(conductors.into_inner().into_iter());
    let keep = std::sync::Mutex::new(Vec::new());
    let keep = &keep;
    futures::stream::iter(iter)
        .for_each_concurrent(20, |(cell, conductor)| {
            let agents = agents.clone();
            async move {
                for agent in agents {
                    if agent == *cell.agent_pubkey() {
                        continue;
                    }
                    let _: ActionHash = conductor
                        .call(
                            &cell.zome(TestWasm::Create),
                            "call_create_entry_remotely_no_rec",
                            agent,
                        )
                        .await;
                }
                keep.lock().unwrap().push(conductor);
            }
        })
        .await;

    // Ensure that all the create requests were received and published.
    await_consistency(60, cells.iter()).await.unwrap();
}

// TODO - rewrite all these tests to use local sweettest

/*
#[test_case(2, 1, 1)]
#[test_case(5, 1, 1)]
#[test_case(1, 5, 5)]
#[test_case(5, 5, 5)]
#[test_case(1, 10, 1)]
#[test_case(1, 1, 1)]
#[test_case(10, 10, 10)]
#[test_case(8, 8, 8)]
#[test_case(10, 10, 1)]
#[ignore = "Don't want network tests running on ci"]
fn conductors_local_gossip(num_committers: usize, num_conductors: usize, new_conductors: usize) {
    let mut network = KitsuneP2pConfig::empty();
    network.transport_pool = vec![TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    let f = conductors_gossip_inner(
        num_committers,
        num_conductors,
        new_conductors,
        network,
        true,
    );
    tokio_helper::block_forever_on(f);
}

#[test_case(2, 1, 1)]
#[test_case(5, 1, 1)]
#[test_case(1, 5, 5)]
#[test_case(5, 5, 5)]
#[test_case(1, 10, 1)]
#[test_case(1, 1, 1)]
#[test_case(10, 10, 10)]
#[test_case(8, 8, 8)]
#[test_case(10, 10, 1)]
#[ignore = "Don't want network tests running on ci"]
fn conductors_boot_gossip(num_committers: usize, num_conductors: usize, new_conductors: usize) {
    let mut network = KitsuneP2pConfig::empty();
    network.bootstrap_service = Some(url2::url2!("https://bootstrap-staging.holo.host"));
    network.transport_pool = vec![TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    let f = conductors_gossip_inner(
        num_committers,
        num_conductors,
        new_conductors,
        network,
        false,
    );
    tokio_helper::block_forever_on(f);
}

#[test_case(2, 1, 1)]
#[test_case(5, 1, 1)]
#[test_case(1, 5, 5)]
#[test_case(5, 5, 5)]
#[test_case(1, 10, 1)]
#[test_case(1, 1, 1)]
#[test_case(10, 10, 10)]
#[test_case(8, 8, 8)]
#[test_case(10, 10, 1)]
#[ignore = "Don't want network tests running on ci"]
fn conductors_local_boot_gossip(
    num_committers: usize,
    num_conductors: usize,
    new_conductors: usize,
) {
    let mut network = KitsuneP2pConfig::empty();
    network.bootstrap_service = Some(url2::url2!("http://localhost:8787"));
    network.transport_pool = vec![TransportConfig::Quic {
        bind_to: None,
        override_host: None,
        override_port: None,
    }];
    let f = conductors_gossip_inner(
        num_committers,
        num_conductors,
        new_conductors,
        network,
        false,
    );
    tokio_helper::block_forever_on(f);
}

#[test_case(2, 1, 1)]
#[test_case(5, 1, 1)]
#[test_case(1, 5, 5)]
#[test_case(5, 5, 5)]
#[test_case(1, 10, 1)]
#[test_case(1, 1, 1)]
#[test_case(10, 10, 10)]
#[test_case(8, 8, 8)]
#[test_case(10, 10, 1)]
#[ignore = "Don't want network tests running on ci"]
fn conductors_remote_gossip(num_committers: usize, num_conductors: usize, new_conductors: usize) {
    let mut network = KitsuneP2pConfig::empty();
    let transport = TransportConfig::Quic {
        bind_to: None,
        override_port: None,
        override_host: None,
    };
    let proxy_config = if let Some(proxy_addr) = std::env::var_os("KIT_PROXY") {
        holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient {
            // Real proxy
            proxy_url: url2::url2!("{}", proxy_addr.into_string().unwrap()),
        }
    } else {
        holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient{
            // Real proxy
            proxy_url: url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/proxy.holochain.org/p/5778/--"),
            // Local proxy
            // proxy_url: url2::url2!("kitsune-proxy://h5_sQGIdBB7OnWVc1iuYZ-QUzb0DowdCA73PA0oOcv4/kitsune-quic/h/192.168.1.6/p/58451/--"),
            // Other machine proxy
            // proxy_url: url2::url2!("kitsune-proxy://h5_sQGIdBB7OnWVc1iuYZ-QUzb0DowdCA73PA0oOcv4/kitsune-quic/h/192.168.1.68/p/58451/--"),
        }
    };

    network.transport_pool = vec![TransportConfig::Proxy {
        sub_transport: transport.into(),
        proxy_config,
    }];
    let f = conductors_gossip_inner(
        num_committers,
        num_conductors,
        new_conductors,
        network,
        true,
    );
    tokio_helper::block_forever_on(f);
}

#[test_case(2, 1, 1)]
#[test_case(5, 1, 1)]
#[test_case(1, 5, 5)]
#[test_case(5, 5, 5)]
#[test_case(1, 10, 1)]
#[test_case(1, 1, 1)]
#[test_case(10, 10, 10)]
#[test_case(8, 8, 8)]
#[test_case(10, 10, 1)]
#[ignore = "Don't want network tests running on ci"]
fn conductors_remote_boot_gossip(
    num_committers: usize,
    num_conductors: usize,
    new_conductors: usize,
) {
    let mut network = KitsuneP2pConfig::empty();
    let transport = TransportConfig::Quic {
        bind_to: None,
        override_port: None,
        override_host: None,
    };
    network.bootstrap_service = Some(url2::url2!("https://bootstrap-staging.holo.host/"));
    let proxy_config = holochain_p2p::kitsune_p2p::ProxyConfig::RemoteProxyClient{
        proxy_url: url2::url2!("kitsune-proxy://CIW6PxKxsPPlcuvUCbMcKwUpaMSmB7kLD8xyyj4mqcw/kitsune-quic/h/proxy.holochain.org/p/5778/--"),
    };
    network.transport_pool = vec![TransportConfig::Proxy {
        sub_transport: transport.into(),
        proxy_config,
    }];
    let f = conductors_gossip_inner(
        num_committers,
        num_conductors,
        new_conductors,
        network,
        false,
    );
    tokio_helper::block_forever_on(f);
}

async fn conductors_gossip_inner(
    num_committers: usize,
    num_conductors: usize,
    new_conductors: usize,
    network: KitsuneP2pConfig,
    share_peers: bool,
) {
    holochain_trace::test_run();
    let network_seed = nanoid::nanoid!().to_string();

    let zomes = vec![TestWasm::Create];
    let handles = setup(
        zomes.clone(),
        Some(network.clone()),
        num_committers,
        network_seed.clone(),
    )
    .await;

    let actions = init_all(&handles[..]).await;

    let second_handles = setup(
        zomes.clone(),
        Some(network.clone()),
        num_conductors,
        network_seed.clone(),
    )
    .await;

    let mut envs = Vec::with_capacity(handles.len() + second_handles.len());
    for h in handles.iter().chain(second_handles.iter()) {
        let space = h.cell_id.dna_hash();
        envs.push(h.get_p2p_db(space));
    }

    if share_peers {
        exchange_peer_info(envs.clone()).await;
    }

    // for _ in 0..600 {
    //     check_peers(envs.clone());
    //     tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    // }

    let all_handles = handles
        .iter()
        .chain(second_handles.iter())
        .collect::<Vec<_>>();

    // 3 ops per create plus 7 for genesis + 2 for init + 2 for cap
    let mut expected_count = num_committers * (3 + 7 + 2 + 2) + num_conductors * 7;
    for (i, handle) in second_handles.iter().enumerate() {
        check_gossip(handle, &all_handles, &actions, expected_count, line!(), i).await;
        // Add 4 ops for each init
        expected_count += 4;
    }

    shutdown(handles).await;

    let third_handles = setup(
        zomes.clone(),
        Some(network.clone()),
        new_conductors,
        network_seed,
    )
    .await;

    let mut envs = Vec::with_capacity(third_handles.len() + second_handles.len());
    for h in third_handles.iter().chain(second_handles.iter()) {
        let space = h.cell_id.dna_hash();
        envs.push(h.get_p2p_db(space));
    }

    if share_peers {
        exchange_peer_info(envs.clone()).await;
    }

    let all_handles = third_handles
        .iter()
        .chain(second_handles.iter())
        .collect::<Vec<_>>();

    expected_count += new_conductors * 7;
    for (i, handle) in third_handles.iter().enumerate() {
        check_gossip(handle, &all_handles, &actions, expected_count, line!(), i).await;
        // Add 4 ops for each init
        expected_count += 4;
    }

    shutdown(second_handles).await;

    let all_handles = third_handles.iter().collect::<Vec<_>>();

    for (i, handle) in third_handles.iter().enumerate() {
        check_gossip(handle, &all_handles, &actions, expected_count, line!(), i).await;
    }

    shutdown(third_handles).await;
}

async fn init_all(handles: &[TestHandle]) -> Vec<ActionHash> {
    let mut futures = Vec::with_capacity(handles.len());
    for (i, h) in handles.iter().cloned().enumerate() {
        let f = async move {
            let large_msg = std::iter::repeat(b"a"[0]).take(20_000).collect::<Vec<_>>();
            let invocation = new_zome_call(
                h.keystore(),
                &h.cell_id,
                "create_post",
                Post(format!("{}{}", i, String::from_utf8_lossy(&large_msg))),
                TestWasm::Create,
            )
            .await
            .unwrap();
            h.call_zome(invocation).await.unwrap().unwrap()
        };
        let f = tokio::task::spawn(f);
        futures.push(f);
    }
    let mut actions = Vec::with_capacity(handles.len());
    for f in futures {
        let result = f.await.unwrap();
        let result: ActionHash = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .decode()
            .unwrap();
        actions.push(result);
    }
    actions
}

async fn check_gossip(
    handle: &TestHandle,
    all_handles: &[&TestHandle],
    posts: &[ActionHash],
    expected_count: usize,
    line: u32,
    i: usize,
) {
    const NUM_ATTEMPTS: usize = 600;
    const DELAY_PER_ATTEMPT: std::time::Duration = std::time::Duration::from_millis(100);

    let mut others = Vec::with_capacity(all_handles.len());
    for other in all_handles {
        let other = other.get_dht_db(other.cell_id.dna_hash()).unwrap().into();
        others.push(other);
    }
    let others_ref = others.iter().collect::<Vec<_>>();

    wait_for_integration_with_others(
        &handle.get_dht_db(handle.cell_id.dna_hash()).unwrap(),
        &others_ref,
        expected_count,
        NUM_ATTEMPTS,
        DELAY_PER_ATTEMPT.clone(),
        None,
    )
    .await;
    for hash in posts {
        let invocation = new_zome_call(
            handle.keystore(),
            &handle.cell_id,
            "get_post",
            hash,
            TestWasm::Create,
        )
        .await
        .unwrap();
        let result = handle.call_zome(invocation).await.unwrap().unwrap();
        let result: Option<Record> = unwrap_to::unwrap_to!(result => ZomeCallResponse::Ok)
            .decode()
            .unwrap();
        let s = debug_span!("check_gossip", ?line, ?i, ?hash);
        let _g = s.enter();
        tracing::debug!("Checking hash {:?} for {}", hash, i);
        tracing::debug!(?result);
        assert_matches!(result, Some(_));
    }
}

#[cfg_attr(feature = "instrument", tracing::instrument(skip(envs)))]
async fn check_peers(envs: Vec<DbWrite<DbKindP2pAgents>>) {
    for (i, a) in envs.iter().enumerate() {
        let peers = all_agent_infos(a.clone().into()).await.unwrap();
        let num_peers = peers.len();
        let peers = peers
            .into_iter()
            .map(|a| a.agent.clone())
            .collect::<Vec<_>>();
        tracing::debug!(?i, ?num_peers, ?peers);
    }
}

#[derive(Shrinkwrap, Clone)]
struct TestHandle {
    #[shrinkwrap(main_field)]
    handle: ConductorHandle,
    cell_id: CellId,
    _db_dir: Arc<TempDir>,
}

impl TestHandle {
    async fn shutdown(self) {
        self.handle.shutdown().await.unwrap().unwrap();
    }
}

async fn shutdown(handles: Vec<TestHandle>) {
    for h in handles {
        h.shutdown().await;
    }
}

async fn setup(
    zomes: Vec<TestWasm>,
    network: Option<KitsuneP2pConfig>,
    num_conductors: usize,
    network_seed: NetworkSeed,
) -> Vec<TestHandle> {
    let dna_file = DnaFile::new(
        DnaDef {
            name: "conductor_test".to_string(),
            modifiers: DnaModifiers {
                network_seed,
                properties: SerializedBytes::try_from(()).unwrap(),
            },
            integrity_zomes: zomes
                .clone()
                .into_iter()
                .map(TestZomes::from)
                .map(|z| z.integrity.into_inner())
                .collect(),
            coordinator_zomes: zomes
                .clone()
                .into_iter()
                .map(TestZomes::from)
                .map(|z| z.coordinator.into_inner())
                .collect(),
        },
        zomes.into_iter().map(Into::into),
    )
    .await;

    let mut handles = Vec::with_capacity(num_conductors);
    for _ in 0..num_conductors {
        let dnas = vec![dna_file.clone()];
        let (_db_dir, _, handle) =
            setup_app_with_network(vec![], vec![], network.clone().unwrap_or_default()).await;

        let agent_key = AgentPubKey::new_random(handle.keystore()).await.unwrap();
        let cell_id = CellId::new(dna_file.dna_hash().to_owned(), agent_key.clone());
        let app = InstalledCell::new(cell_id.clone(), "cell_handle".into());
        install_app("test_app", vec![(app, None)], dnas.clone(), handle.clone()).await;
        handles.push(TestHandle {
            _db_dir: Arc::new(_db_dir),
            cell_id,
            handle,
        });
    }
    handles
}
*/
