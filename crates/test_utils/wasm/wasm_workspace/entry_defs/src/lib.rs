use hdk3::prelude::*;

#[hdk_entry(id = "post")]
struct Post;

#[hdk_entry(id = "comment", visibility = "private")]
struct Comment;

entry_defs![Post::entry_def(), Comment::entry_def()];

fn entry_id_to_index<I: Into<EntryDefId>>(id: I) -> ExternResult<Option<EntryDefIndex>> {
    match entry_defs_hdk_extern(())? {
        EntryDefsCallbackResult::Defs(entry_defs) => {
            match entry_defs.entry_def_id_position(id.into()) {
                Some(index) => Ok(Some(EntryDefIndex::from(index as u8))),
                None => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn entry_id_to_app_entry_type<I: Into<EntryDefId>>(id: I) -> ExternResult<Option<AppEntryType>> {
    match entry_defs_hdk_extern(())? {
        EntryDefsCallbackResult::Defs(entry_defs) => {
            match entry_defs.entry_def_id_position(id.into()) {
                Some(index) => {
                    let zome_id = zome_info!()?.zome_id;
                    let visibility = entry_defs[index].visibility;
                    Ok(Some(AppEntryType::new(
                        (index as u8).into(),
                        zome_id,
                        visibility,
                    )))
                }
                None => Ok(None),
            }
        }
        _ => Ok(None),
    }
}
