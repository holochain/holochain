use hdk3::prelude::*;
use holochain::test_utils::cool::{CoolAgents, CoolConductor, CoolDnaFile, MaybeElement};
use holochain::test_utils::display_agent_infos;
use holochain::test_utils::wait_for_integration_10s;
use holochain::test_utils::WaitOps;
use holochain_types::dna::zome::inline_zome::InlineZome;
use holochain_zome_types::element::ElementEntry;

#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes, derive_more::From)]
#[serde(transparent)]
#[repr(transparent)]
struct AppString(String);

fn simple_crud_zome() -> InlineZome {
    let string_entry_def = EntryDef::default_with_id("string");
    let unit_entry_def = EntryDef::default_with_id("unit");

    InlineZome::new_unique(vec![string_entry_def.clone(), unit_entry_def.clone()])
        .callback("create_string", move |api, s: String| {
            let entry_def_id: EntryDefId = string_entry_def.id.clone();
            let entry = Entry::app(AppString::from(s).try_into().unwrap()).unwrap();
            let hash = api.create((entry_def_id, entry))?;
            Ok(hash)
        })
        .callback("create_unit", move |api, ()| {
            let entry_def_id: EntryDefId = unit_entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create((entry_def_id, entry))?;
            Ok(hash)
        })
        .callback("delete", move |api, header_hash: HeaderHash| {
            let hash = api.delete(header_hash)?;
            Ok(hash)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get((hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
        .callback("read_entry", |api, hash: EntryHash| {
            api.get((hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
}

#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
async fn inline_zome_2_agents_1_dna() -> anyhow::Result<()> {
    // Bundle the single zome into a DnaFile
    let (dna_file, _) = CoolDnaFile::unique_from_inline_zome("zome1", simple_crud_zome()).await?;

    // Create a Conductor
    let mut conductor = CoolConductor::from_standard_config().await;

    // Get two agents
    let (alice, bobbo) = CoolAgents::two(conductor.keystore()).await;

    // Install DNA and install and activate apps in conductor
    let apps = conductor
        .setup_app_for_agents("app", &[alice.clone(), bobbo.clone()], &[dna_file])
        .await;

    let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductor
        .call(&alice.zome("zome1"), "create_unit", ())
        .await;

    // Wait long enough for Bob to receive gossip
    wait_for_integration_10s(
        bobbo.env(),
        WaitOps::start() + WaitOps::cold_start() + WaitOps::ENTRY,
    )
    .await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: MaybeElement = conductor.call(&bobbo.zome("zome1"), "read", hash).await;
    let element = element
        .0
        .expect("Element was None: bobbo couldn't `get` it");

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
async fn inline_zome_3_agents_2_dnas() -> anyhow::Result<()> {
    observability::test_run().ok();
    let mut conductor = CoolConductor::from_standard_config().await;

    let (dna_foo, _) = CoolDnaFile::unique_from_inline_zome("foozome", simple_crud_zome()).await?;
    let (dna_bar, _) = CoolDnaFile::unique_from_inline_zome("barzome", simple_crud_zome()).await?;

    let agents = CoolAgents::get(conductor.keystore(), 3).await;

    let apps = conductor
        .setup_app_for_agents("app", &agents, &[dna_foo, dna_bar])
        .await;

    let ((alice_foo, alice_bar), (bobbo_foo, bobbo_bar), (_carol_foo, carol_bar)) =
        apps.into_tuples();

    assert_eq!(alice_foo.agent_pubkey(), alice_bar.agent_pubkey());
    assert_eq!(bobbo_foo.agent_pubkey(), bobbo_bar.agent_pubkey());
    assert_ne!(alice_foo.agent_pubkey(), bobbo_foo.agent_pubkey());

    //////////////////////
    // END SETUP

    let hash_foo: HeaderHash = conductor
        .call(&alice_foo.zome("foozome"), "create_unit", ())
        .await;
    let hash_bar: HeaderHash = conductor
        .call(&alice_bar.zome("barzome"), "create_unit", ())
        .await;

    // Two different DNAs, so HeaderHashes should be different.
    assert_ne!(hash_foo, hash_bar);

    // Wait long enough for others to receive gossip
    for env in [bobbo_foo.env(), carol_bar.env()].iter() {
        wait_for_integration_10s(
            env,
            WaitOps::start() * 1 + WaitOps::cold_start() * 2 + WaitOps::ENTRY * 1,
        )
        .await;
    }

    // Verify that bobbo can run "read" on his cell and get alice's Header
    // on the "foo" DNA
    let element: MaybeElement = conductor
        .call(&bobbo_foo.zome("foozome"), "read", hash_foo)
        .await;
    let element = element
        .0
        .expect("Element was None: bobbo couldn't `get` it");
    assert_eq!(element.header().author(), alice_foo.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    // Verify that carol can run "read" on her cell and get alice's Header
    // on the "bar" DNA
    // Let's do it with the CoolZome instead of the CoolCell too, for fun
    let element: MaybeElement = conductor
        .call(&carol_bar.zome("barzome"), "read", hash_bar)
        .await;
    let element = element
        .0
        .expect("Element was None: carol couldn't `get` it");
    assert_eq!(element.header().author(), alice_bar.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
#[ignore = "Needs to be completed when HolochainP2pEvents is accessible"]
async fn invalid_cell() -> anyhow::Result<()> {
    observability::test_run().ok();
    let mut conductor = CoolConductor::from_standard_config().await;

    let (dna_foo, _) = CoolDnaFile::unique_from_inline_zome("foozome", simple_crud_zome()).await?;
    let (dna_bar, _) = CoolDnaFile::unique_from_inline_zome("barzome", simple_crud_zome()).await?;

    // let agents = CoolAgents::get(conductor.keystore(), 2).await;

    let _app_foo = conductor.setup_app("foo", &[dna_foo]).await;

    let _app_bar = conductor.setup_app("bar", &[dna_bar]).await;

    // Give small amount of time for cells to join the network
    tokio::time::delay_for(std::time::Duration::from_millis(500)).await;

    tracing::debug!(dnas = ?conductor.list_dnas().await.unwrap());
    tracing::debug!(cell_ids = ?conductor.list_cell_ids().await.unwrap());
    tracing::debug!(apps = ?conductor.list_active_apps().await.unwrap());

    display_agent_infos(&conductor).await;

    // Can't finish this test because there's no way to construct HolochainP2pEvents
    // and I can't directly call query on the conductor because it's private.

    Ok(())
}

#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
async fn get_deleted() -> anyhow::Result<()> {
    observability::test_run().ok();
    // Bundle the single zome into a DnaFile
    let (dna_file, _) = CoolDnaFile::unique_from_inline_zome("zome1", simple_crud_zome()).await?;

    // Create a Conductor
    let mut conductor = CoolConductor::from_config(Default::default()).await;

    // Install DNA and install and activate apps in conductor
    let alice = conductor
        .setup_app("app", &[dna_file])
        .await
        .into_cells()
        .into_iter()
        .next()
        .unwrap();
    // let ((alice,), (bobbo,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = conductor
        .call(&alice.zome("zome1"), "create_unit", ())
        .await;
    let mut expected_count = WaitOps::start() + WaitOps::ENTRY;

    wait_for_integration_10s(alice.env(), expected_count).await;

    let element: MaybeElement = conductor
        .call(&alice.zome("zome1"), "read", hash.clone())
        .await;
    let element = element
        .0
        .expect("Element was None: bobbo couldn't `get` it");

    assert_eq!(element.header().author(), alice.agent_pubkey());
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );
    let entry_hash = element.header().entry_hash().unwrap();

    let _: HeaderHash = conductor
        .call(&alice.zome("zome1"), "delete", hash.clone())
        .await;

    expected_count += WaitOps::DELETE;
    wait_for_integration_10s(alice.env(), expected_count).await;

    let element: MaybeElement = conductor
        .call(&alice.zome("zome1"), "read_entry", entry_hash)
        .await;
    assert!(element.0.is_none());

    Ok(())
}
