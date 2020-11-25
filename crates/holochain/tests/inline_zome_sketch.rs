use futures::StreamExt;
use hdk3::prelude::*;
use holochain::nucleus::dna::zome::inline_zome::InlineZome;
use holochain::nucleus::dna::zome::ZomeDef;
use holochain::nucleus::dna::DnaDefBuilder;
use holochain::nucleus::dna::DnaFile;
use holochain::nucleus::ribosome::ZomeCallInvocation;
use holochain::{conductor::Conductor, prelude::Zome};
use holochain_keystore::KeystoreSender;
use holochain_state::test_utils::test_environments;
use holochain_types::app::InstalledCell;

#[tokio::test(threaded_scheduler)]
#[ignore = "WIP"]
#[allow(unused_variables, unreachable_code)]
async fn one() -> anyhow::Result<()> {
    let envs = test_environments();
    let zome_def: ZomeDef = InlineZome::new("")
        .callback("create", |api, ()| {
            let entry_def_id: EntryDefId = todo!();
            let entry: Entry = todo!();
            let hash = api.create_entry(entry_def_id, entry)?;
            Ok(())
        })
        .callback("read", |api, hash: EntryHash| {
            api.get(hash, GetOptions::default())
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
