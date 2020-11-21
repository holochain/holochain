use hdk3::prelude::*;
use holochain::conductor::Conductor;
use holochain_state::test_utils::test_environments;
use holochain_types::dna::{
    zome::{inline_zome::InlineZome, ZomeDef},
    DnaDefBuilder, DnaFile,
};

#[tokio::test(threaded_scheduler)]
async fn one() -> anyhow::Result<()> {
    let envs = test_environments();
    let conductor = Conductor::builder().test(&envs).await?;
    let zome: ZomeDef = InlineZome::new("")
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
    let dna = DnaDefBuilder::default()
        .zomes(vec![("zome1".into(), zome.into())])
        .random_uuid()
        .build()
        .unwrap();
    let dna_file = DnaFile::new(dna, vec![]).await?;
    Ok(())
}
