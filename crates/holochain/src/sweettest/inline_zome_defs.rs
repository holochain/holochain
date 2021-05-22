//! Some useful reusable inline zomes

use holochain_types::prelude::*;

/// The simplest possible zome which can Create and Read a (unit) entry type
pub fn simple_crud_zome() -> InlineZome {
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
