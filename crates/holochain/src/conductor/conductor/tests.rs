use super::Conductor;
use super::ConductorState;
use super::*;
use crate::conductor::dna_store::MockDnaStore;
use holochain_lmdb::test_utils::test_environments;
use holochain_types::test_utils::fake_cell_id;
use matches::assert_matches;

#[tokio::test(threaded_scheduler)]
async fn can_update_state() {
    let envs = test_environments();
    let dna_store = MockDnaStore::new();
    let keystore = envs.conductor().keystore().clone();
    let holochain_p2p = holochain_p2p::stub_network().await;
    let conductor = Conductor::new(
        envs.conductor(),
        envs.wasm(),
        envs.p2p(),
        dna_store,
        keystore,
        envs.tempdir().path().to_path_buf().into(),
        holochain_p2p,
    )
    .await
    .unwrap();
    let state = conductor.get_state().await.unwrap();
    assert_eq!(state, ConductorState::default());

    let cell_id = fake_cell_id(1);
    let installed_cell = InstalledCell::new(cell_id.clone(), "handle".to_string());

    conductor
        .update_state(|mut state| {
            state
                .inactive_apps
                .insert("fake app".to_string(), vec![installed_cell]);
            Ok(state)
        })
        .await
        .unwrap();
    let state = conductor.get_state().await.unwrap();
    assert_eq!(
        state.inactive_apps.values().collect::<Vec<_>>()[0]
            .into_iter()
            .map(|c| c.as_id().clone())
            .collect::<Vec<_>>()
            .as_slice(),
        &[cell_id]
    );
}

/// App can't be installed if another app is already installed under the
/// same InstalledAppId
#[tokio::test(threaded_scheduler)]
async fn app_ids_are_unique() {
    let environments = test_environments();
    let dna_store = MockDnaStore::new();
    let holochain_p2p = holochain_p2p::stub_network().await;
    let mut conductor = Conductor::new(
        environments.conductor(),
        environments.wasm(),
        environments.p2p(),
        dna_store,
        environments.keystore().clone(),
        environments.tempdir().path().to_path_buf().into(),
        holochain_p2p,
    )
    .await
    .unwrap();

    let cell_id = fake_cell_id(1);
    let installed_cell = InstalledCell::new(cell_id.clone(), "handle".to_string());
    let app = InstalledApp {
        installed_app_id: "id".to_string(),
        cell_data: vec![installed_cell],
        active: false,
    };

    conductor.add_inactive_app_to_db(app.clone()).await.unwrap();

    assert_matches!(
        conductor.add_inactive_app_to_db(app.clone()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if id == "id".to_string()
    );

    //- it doesn't matter whether the app is active or inactive
    conductor
        .activate_app_in_db("id".to_string())
        .await
        .unwrap();

    assert_matches!(
        conductor.add_inactive_app_to_db(app.clone()).await,
        Err(ConductorError::AppAlreadyInstalled(id))
        if id == "id".to_string()
    );
}

#[tokio::test(threaded_scheduler)]
async fn can_set_fake_state() {
    let envs = test_environments();
    let state = ConductorState::default();
    let conductor = ConductorBuilder::new()
        .fake_state(state.clone())
        .test(&envs)
        .await
        .unwrap();
    assert_eq!(state, conductor.get_state_from_handle().await.unwrap());
}

#[tokio::test(threaded_scheduler)]
async fn proxy_tls_with_test_keystore() {
    use ghost_actor::GhostControlSender;

    observability::test_run().ok();

    let keystore1 = spawn_test_keystore().await.unwrap();
    let keystore2 = spawn_test_keystore().await.unwrap();

    if let Err(e) = proxy_tls_inner(keystore1.clone(), keystore2.clone()).await {
        panic!("{:#?}", e);
    }

    let _ = keystore1.ghost_actor_shutdown_immediate().await;
    let _ = keystore2.ghost_actor_shutdown_immediate().await;
}

async fn proxy_tls_inner(
    keystore1: KeystoreSender,
    keystore2: KeystoreSender,
) -> anyhow::Result<()> {
    use ghost_actor::GhostControlSender;
    use kitsune_p2p::dependencies::*;
    use kitsune_p2p_proxy::*;
    use kitsune_p2p_types::transport::*;

    let (cert_digest, cert, cert_priv_key) = keystore1.get_or_create_first_tls_cert().await?;

    let tls_config1 = TlsConfig {
        cert,
        cert_priv_key,
        cert_digest,
    };

    let (cert_digest, cert, cert_priv_key) = keystore2.get_or_create_first_tls_cert().await?;

    let tls_config2 = TlsConfig {
        cert,
        cert_priv_key,
        cert_digest,
    };

    let proxy_config =
        ProxyConfig::local_proxy_server(tls_config1, AcceptProxyCallback::reject_all());
    let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
    let (bind1, mut evt1) = spawn_kitsune_proxy_listener(proxy_config, bind, evt).await?;
    tokio::task::spawn(async move {
        while let Some(evt) = evt1.next().await {
            match evt {
                TransportEvent::IncomingChannel(_, mut write, read) => {
                    println!("YOOTH");
                    let data = read.read_to_end().await;
                    let data = String::from_utf8_lossy(&data);
                    let data = format!("echo: {}", data);
                    write.write_and_close(data.into_bytes()).await?;
                }
            }
        }
        TransportResult::Ok(())
    });
    let url1 = bind1.bound_url().await?;
    println!("{:?}", url1);

    let proxy_config =
        ProxyConfig::local_proxy_server(tls_config2, AcceptProxyCallback::reject_all());
    let (bind, evt) = kitsune_p2p_types::transport_mem::spawn_bind_transport_mem().await?;
    let (bind2, _evt2) = spawn_kitsune_proxy_listener(proxy_config, bind, evt).await?;
    println!("{:?}", bind2.bound_url().await?);

    let (_url, mut write, read) = bind2.create_channel(url1).await?;
    write.write_and_close(b"test".to_vec()).await?;
    let data = read.read_to_end().await;
    let data = String::from_utf8_lossy(&data);
    assert_eq!("echo: test", data);

    let _ = bind1.ghost_actor_shutdown_immediate().await;
    let _ = bind2.ghost_actor_shutdown_immediate().await;

    Ok(())
}
