use hdk::prelude::*;
use holochain::conductor::config::ConductorConfig;
use holochain::test_utils::host_fn_caller::Post;
use holochain::test_utils::show_authored;
use holochain::test_utils::sweetest::SweetNetwork;
use holochain::test_utils::sweetest::{SweetConductorBatch, SweetDnaFile};
use holochain::test_utils::wait_for_integration_1m;
use holochain::test_utils::wait_for_integration_with_others_10s;
use holochain::test_utils::WaitOps;
use holochain_types::dna::zome::inline_zome::InlineZome;
use holochain_zome_types::element::ElementEntry;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

fn simple_crud_zome() -> InlineZome {
    let entry_def = EntryDef::default_with_id("entrydef");

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(EntryWithDefId::new(entry_def_id, entry))?;
            Ok(hash)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get(GetInput::new(hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
}

fn invalid_cell_zome() -> InlineZome {
    let entry_def = EntryDef::default_with_id("entrydef");

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, entry: Post| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(entry.try_into().unwrap()).unwrap();
            let hash = api.create(EntryWithDefId::new(entry_def_id, entry))?;
            Ok(hash)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get(GetInput::new(hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
}

// TODO [ B-03669 ]: make much less verbose
#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
async fn multi_conductor() -> anyhow::Result<()> {
    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let mut conductors = SweetConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", simple_crud_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await;
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (_carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0].call(&alice.zome("zome1"), "create", ()).await;

    // Wait long enough for Bob to receive gossip
    wait_for_integration_1m(
        bobbo.env(),
        WaitOps::start() * 1 + WaitOps::cold_start() * 2 + WaitOps::ENTRY * 1,
    )
    .await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductors[1].call(&bobbo.zome("zome1"), "read", hash).await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
async fn invalid_cell() -> anyhow::Result<()> {
    let _g = observability::test_run().ok();
    const NUM_CONDUCTORS: usize = 3;

    let network = SweetNetwork::env_var_proxy().unwrap_or_else(|| {
        info!("KIT_PROXY not set using local quic network");
        SweetNetwork::local_quic()
    });
    let mut config = ConductorConfig::default();
    config.network = Some(network);

    let mut conductors = SweetConductorBatch::from_config(NUM_CONDUCTORS, config).await;

    let (dna_file, _) = SweetDnaFile::unique_from_inline_zome("zome1", invalid_cell_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await;
    conductors.exchange_peer_info().await;

    let ((alice,), (bobbo,), (carol,)) = apps.into_tuples();
    let alice_env = alice.env();
    let bob_env = bobbo.env();
    let carol_env = carol.env();
    let envs = vec![alice_env, bob_env, carol_env];

    conductors[1].shutdown().await;

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductors[0]
        .call(&alice.zome("zome1"), "create", Post("1".to_string()))
        .await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: Option<Element> = conductors[0]
        .call(&alice.zome("zome1"), "read", hash.clone())
        .await;
    let element = element.expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bobbo sees matches what alice committed
    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(Post("1".to_string()).try_into().unwrap()).unwrap())
    );
    conductors[1].startup().await;
    let _: Option<Element> = conductors[1].call(&bobbo.zome("zome1"), "read", hash).await;

    // Take both other conductors offline and commit a hash they don't have
    // then bring them back with the original offline.
    conductors[0].shutdown().await;
    conductors[2].shutdown().await;

    let hash: HeaderHash = conductors[1]
        .call(&bobbo.zome("zome1"), "create", Post("2".to_string()))
        .await;
    conductors[1].shutdown().await;
    conductors[0].startup().await;
    let r: Option<Element> = conductors[0]
        .call(&alice.zome("zome1"), "read", hash.clone())
        .await;
    assert!(r.is_none());
    conductors[2].startup().await;
    let r: Option<Element> = conductors[2]
        .call(&carol.zome("zome1"), "read", hash.clone())
        .await;
    assert!(r.is_none());
    conductors[1].startup().await;

    let _: HeaderHash = conductors[0]
        .call(&alice.zome("zome1"), "create", Post("3".to_string()))
        .await;
    let _: HeaderHash = conductors[1]
        .call(&bobbo.zome("zome1"), "create", Post("4".to_string()))
        .await;
    let _: HeaderHash = conductors[2]
        .call(&carol.zome("zome1"), "create", Post("5".to_string()))
        .await;

    let expected_count = WaitOps::start() * 3 + WaitOps::ENTRY * 5;
    // wait_for_integration_1m(&alice.env().await, expected_count).await;
    show_authored(&envs);
    // wait_for_integration_1m(&carol_env, expected_count).await;
    wait_for_integration_with_others_10s(&alice_env, &envs, expected_count).await;
    let r: Option<Element> = conductors[0]
        .call(&alice.zome("zome1"), "read", hash.clone())
        .await;
    assert!(r.is_some());
    Ok(())
}
