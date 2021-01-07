use hdk3::prelude::*;
use holochain::test_utils::cool::MaybeElement;
use holochain::test_utils::cool::{CoolConductorBatch, CoolDnaFile};
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
            api.get((hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
}

// TODO [ B-03669 ]: make much less verbose
#[tokio::test(threaded_scheduler)]
#[cfg(feature = "test_utils")]
async fn multi_conductor() -> anyhow::Result<()> {
    const NUM_CONDUCTORS: usize = 3;

    let conductors = CoolConductorBatch::from_standard_config(NUM_CONDUCTORS).await;

    let (dna_file, _) = CoolDnaFile::unique_from_inline_zome("zome1", simple_crud_zome())
        .await
        .unwrap();

    let apps = conductors.setup_app("app", &[dna_file]).await;
    conductors.exchange_peer_info().await;

    // TODO: write better helper
    let ((alice,), (bobbo,), (_carol,)) = apps.into_tuples();

    // Call the "create" zome fn on Alice's app
    let hash: HeaderHash = alice.call("zome1", "create", ()).await;

    // Wait long enough for Bob to receive gossip (TODO: make deterministic)
    tokio::time::delay_for(std::time::Duration::from_millis(5000)).await;

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
