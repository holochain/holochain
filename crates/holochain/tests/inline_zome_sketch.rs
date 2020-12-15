use futures::StreamExt;
use hdk3::prelude::*;
use holochain::conductor::api::ZomeCall;
use holochain::conductor::Conductor;
use holochain::destructure_test_cells;
use holochain::test_utils::test_conductor::MaybeElement;
use holochain::test_utils::test_conductor::TestAgents;
use holochain::test_utils::test_conductor::TestConductorHandle;
use holochain_keystore::KeystoreSender;
use holochain_lmdb::test_utils::test_environments;
use holochain_types::app::InstalledCell;
use holochain_types::dna::zome::inline_zome::InlineZome;
use holochain_types::dna::DnaFile;
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
        .callback("read", |api, hash: HeaderHash| {
            api.get((hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
}

#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
async fn inline_zome_2_agents_1_dna() -> anyhow::Result<()> {
    let envs = test_environments();

    // Bundle the single zome into a DnaFile
    let (dna_file, _) = DnaFile::unique_from_inline_zome("zome1", simple_crud_zome()).await?;

    // Get two agents
    let (alice, bobbo) = TestAgents::two(envs.keystore()).await;

    // Create a Conductor
    let conductor: TestConductorHandle = Conductor::builder().test(&envs).await?.into();

    // Install DNA and install and activate apps in conductor
    let ids = conductor
        .setup_app_for_agents_with_no_membrane_proof(
            "app",
            &[alice.clone(), bobbo.clone()],
            &[dna_file],
        )
        .await;

    let ((alice,), (bobbo,)) = destructure_test_cells!(ids);

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = alice.call("zome1", "create_unit", ()).await;

    // Wait long enough for Bob to receive gossip (TODO: make deterministic)
    tokio::time::delay_for(std::time::Duration::from_millis(500)).await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    let element: MaybeElement = bobbo.call("zome1", "read", hash).await;
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
    let envs = test_environments();
    let conductor: TestConductorHandle = Conductor::builder().test(&envs).await?.into();

    let (dna_foo, _) = DnaFile::unique_from_inline_zome("foozome", simple_crud_zome()).await?;
    let (dna_bar, _) = DnaFile::unique_from_inline_zome("barzome", simple_crud_zome()).await?;

    let agents = TestAgents::get(envs.keystore(), 3).await;

    let ids = conductor
        .setup_app_for_agents_with_no_membrane_proof("app", &agents, &[dna_foo, dna_bar])
        .await;

    let ((alice_foo, alice_bar), (bobbo_foo, bobbo_bar), (_carol_foo, carol_bar)) =
        destructure_test_cells!(ids);

    assert_eq!(alice_foo.agent_pubkey(), alice_bar.agent_pubkey());
    assert_eq!(bobbo_foo.agent_pubkey(), bobbo_bar.agent_pubkey());
    assert_ne!(alice_foo.agent_pubkey(), bobbo_foo.agent_pubkey());

    //////////////////////
    // END SETUP

    let hash_foo: HeaderHash = alice_foo.call("foozome", "create_unit", ()).await;
    let hash_bar: HeaderHash = alice_bar.call("barzome", "create_unit", ()).await;

    // Two different DNAs, so HeaderHashes should be different.
    assert_ne!(hash_foo, hash_bar);

    // Wait long enough for others to receive gossip (TODO: make deterministic)
    tokio::time::delay_for(std::time::Duration::from_millis(500)).await;

    // Verify that bobbo can run "read" on his cell and get alice's Header
    // on the "foo" DNA
    let element: MaybeElement = bobbo_foo.call("foozome", "read", hash_foo).await;
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
    // Let's do it with the TestZome instead of the TestCell too, for fun
    let element: MaybeElement = carol_bar.zome("barzome").call("read", hash_bar).await;
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
