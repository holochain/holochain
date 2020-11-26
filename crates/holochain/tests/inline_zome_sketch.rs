use futures::StreamExt;
use hdk3::prelude::*;
use holochain::conductor::Conductor;
use holochain::core::ribosome::ZomeCallInvocation;
use holochain_keystore::KeystoreSender;
use holochain_state::test_utils::test_environments;
use holochain_types::app::InstalledCell;
use holochain_types::dna::zome::inline_zome::InlineZome;
use holochain_types::dna::zome::{Zome, ZomeDef};
use holochain_types::dna::DnaDefBuilder;
use holochain_types::dna::DnaFile;

#[tokio::test(threaded_scheduler)]
async fn one() -> anyhow::Result<()> {
    let envs = test_environments();
    let entry_def = EntryDef::new(
        "entry".into(),
        Default::default(),
        Default::default(),
        Default::default(),
        Default::default(),
    );
    let zome_def: ZomeDef = InlineZome::new("", vec![entry_def.clone()])
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create((entry_def_id, entry))?;
            Ok(hash)
        })
        .callback("read", |api, hash: EntryHash| {
            api.get((hash.into(), GetOptions::default()))
                .map_err(Into::into)
        })
        .into();
    let zome = Zome::new("zome1".into(), zome_def);
    let dna = DnaDefBuilder::default()
        .zomes(vec![zome.clone().into_inner()])
        .random_uuid()
        .build()
        .unwrap();
    let dna_file = DnaFile::new(dna, vec![]).await?;
    let dna_hash = dna_file.dna_hash().clone();

    let (alice, bobbo) = {
        let mut agents: Vec<AgentPubKey> = agent_stream(envs.keystore()).take(2).collect().await;
        (agents.pop().unwrap(), agents.pop().unwrap())
    };
    let alice_cell_id = CellId::new(dna_hash.clone(), alice.clone());
    let bobbo_cell_id = CellId::new(dna_hash.clone(), bobbo.clone());
    let conductor = Conductor::builder().test(&envs).await?;
    conductor.install_dna(dna_file).await?;
    conductor
        .clone()
        .install_app(
            "app".to_string(),
            vec![
                (
                    InstalledCell::new(alice_cell_id.clone(), "dna".into()),
                    None,
                ),
                (
                    InstalledCell::new(bobbo_cell_id.clone(), "dna".into()),
                    None,
                ),
            ],
        )
        .await?;
    conductor.activate_app("app".to_string()).await?;

    let output = conductor
        .call_zome(ZomeCallInvocation {
            cell_id: alice_cell_id.clone(),
            zome: zome.clone(),
            fn_name: "create".into(),
            payload: ExternInput::new(().try_into().unwrap()),
            cap: None,
            provenance: alice.clone(),
        })
        .await??;
    dbg!(&output);
    let output = conductor
        .call_zome(ZomeCallInvocation {
            cell_id: bobbo_cell_id.clone(),
            zome: zome.clone(),
            fn_name: "read".into(),
            payload: ExternInput::new(().try_into().unwrap()),
            cap: None,
            provenance: bobbo.clone(),
        })
        .await??;
    dbg!(&output);

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
