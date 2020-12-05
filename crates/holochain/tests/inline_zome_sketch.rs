use hdk3::prelude::*;
use holochain::conductor::Conductor;
use holochain::test_utils::{test_agents::TestAgents, test_handle::TestConductorHandle};
use holochain_state::test_utils::test_environments;
use holochain_types::dna::{zome::inline_zome::InlineZome, DnaFile};
use holochain_zome_types::element::ElementEntry;

// TODO: remove once host fns remove SerializedBytes constraint
#[derive(serde::Serialize, serde::Deserialize, Debug, SerializedBytes)]
#[serde(transparent)]
#[repr(transparent)]
struct MaybeElement(Option<Element>);

fn simple_crud_zome() -> InlineZome {
    let entry_def = EntryDef::new(
        "entry".into(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
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
async fn inline_zome_feasibility_test() -> anyhow::Result<()> {
    let envs = test_environments();

    // Bundle the single zome into a DnaFile

    let (dna_file, zome) = DnaFile::unique_from_inline_zome("zome1", simple_crud_zome()).await?;
    let dna_hash = dna_file.dna_hash().clone();
    let zome_name = zome.zome_name();

    // Get two agents

    let (alice, bobbo) = TestAgents::two(envs.keystore()).await;
    let alice_cell_id = CellId::new(dna_hash.clone(), alice.clone());
    let bobbo_cell_id = CellId::new(dna_hash.clone(), bobbo.clone());

    // Create a Conductor

    let conductor: TestConductorHandle = Conductor::builder().test(&envs).await?.into();

    // Install DNA and install and activate apps in conductor

    let _ids = conductor
        .setup_app_for_all_agents_with_no_membrane_proof(
            "app",
            &[dna_file],
            &[alice.clone(), bobbo.clone()],
        )
        .await;

    // Call the "create" zome fn on Alice's app

    let hash: HeaderHash = conductor
        .call_zome_ok_flat(&alice_cell_id, zome_name, "create", None, None, ())
        .await;

    // Wait long enough for Bob to receive gossip

    tokio::time::delay_for(std::time::Duration::from_millis(500)).await;

    // Verify that bob can run "read" on his app and get alice's Header

    let element: MaybeElement = conductor
        .call_zome_ok_flat(&bobbo_cell_id, zome_name, "read", None, None, hash)
        .await;
    let element = element
        .0
        .expect("Element was None: bobbo couldn't `get` it");

    // Assert that the Element bob sees matches what Alice committed

    assert_eq!(*element.header().author(), alice);
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}
