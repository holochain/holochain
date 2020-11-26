use futures::StreamExt;
use hdk3::prelude::*;
use holochain::{conductor::Conductor, core::ribosome::ZomeCallInvocation};
use holochain_keystore::KeystoreSender;
use holochain_state::test_utils::test_environments;
use holochain_types::{
    app::InstalledCell,
    dna::{
        zome::{inline_zome::InlineZome, Zome, ZomeDef},
        DnaDefBuilder, DnaFile,
    },
};
use holochain_zome_types::element::ElementEntry;
use unwrap_to::unwrap_to;

#[tokio::test(threaded_scheduler)]
async fn extremely_verbose_inline_zome_sketch() -> anyhow::Result<()> {
    let envs = test_environments();

    // Create an EntryDef for use in this test

    let entry_def = EntryDef::new(
        "entry".into(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );

    // Create an InlineZome whose callbacks are defined by closures

    let zome_def: ZomeDef = InlineZome::new("", vec![entry_def.clone()])
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
        .into();
    let zome = Zome::new("zome1".into(), zome_def);

    // Bundle the single zome into a DnaFile

    let dna = DnaDefBuilder::default()
        .zomes(vec![zome.clone().into_inner()])
        .random_uuid()
        .build()
        .unwrap();
    let dna_file = DnaFile::new(dna, vec![]).await?;
    let dna_hash = dna_file.dna_hash().clone();

    // Get two agents

    let (alice, bobbo) = {
        let mut agents: Vec<AgentPubKey> = agent_stream(envs.keystore()).take(2).collect().await;
        (agents.pop().unwrap(), agents.pop().unwrap())
    };
    let alice_cell_id = CellId::new(dna_hash.clone(), alice.clone());
    let bobbo_cell_id = CellId::new(dna_hash.clone(), bobbo.clone());

    // Create a Conductor

    let conductor = Conductor::builder().test(&envs).await?;

    // Install the DNA

    conductor.install_dna(dna_file).await?;

    // Install and activate one app for Alice and another for Bob

    conductor
        .clone()
        .install_app(
            "app:alice".to_string(),
            vec![(
                InstalledCell::new(alice_cell_id.clone(), "dna".into()),
                None,
            )],
        )
        .await?;
    conductor
        .clone()
        .install_app(
            "app:bobbo".to_string(),
            vec![(
                InstalledCell::new(bobbo_cell_id.clone(), "dna".into()),
                None,
            )],
        )
        .await?;
    conductor.activate_app("app:alice".to_string()).await?;
    conductor.activate_app("app:bobbo".to_string()).await?;
    conductor.clone().setup_cells().await?;

    // Call the "create" zome fn on Alice's app

    let hash: HeaderHash = {
        let response = conductor
            .call_zome(ZomeCallInvocation {
                cell_id: alice_cell_id.clone(),
                zome: zome.clone(),
                fn_name: "create".into(),
                payload: ExternInput::new(().try_into().unwrap()),
                cap: None,
                provenance: alice.clone(),
            })
            .await??;
        unwrap_to!(response => ZomeCallResponse::Ok)
            .clone()
            .into_inner()
            .try_into()?
    };

    // Wait long enough for Bob to receive gossip

    tokio::time::delay_for(std::time::Duration::from_millis(500)).await;

    // Verify that bob can run "read" on his app and get alice's Header

    let element: Element = {
        let response = conductor
            .call_zome(ZomeCallInvocation {
                cell_id: bobbo_cell_id.clone(),
                zome: zome.clone(),
                fn_name: "read".into(),
                payload: ExternInput::new(hash.try_into().unwrap()),
                cap: None,
                provenance: bobbo.clone(),
            })
            .await??;
        let sb = unwrap_to!(response => ZomeCallResponse::Ok)
            .clone()
            .into_inner();
        holochain_serialized_bytes::decode(sb.bytes())
            .expect("Element was None: bobbo couldn't `get` it")
    };

    // Assert that the Element bob sees matches what Alice committed

    assert_eq!(*element.header().author(), alice);
    assert_eq!(
        *element.entry(),
        ElementEntry::Present(Entry::app(().try_into().unwrap()).unwrap())
    );

    Ok(())
}

fn agent_stream(keystore: KeystoreSender) -> impl futures::Stream<Item = AgentPubKey> {
    use holochain_keystore::KeystoreSenderExt;
    futures::stream::unfold(keystore, |keystore| async {
        let key = keystore
            .generate_sign_keypair_from_pure_entropy()
            .await
            .expect("can generate AgentPubKey");
        Some((key, keystore))
    })
}
