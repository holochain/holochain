//! Collection of commonly used InlineZomes

use holo_hash::*;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::prelude::*;

/// An InlineZome with simple Create and Read operations
pub fn simple_create_read_zome() -> InlineZomeSet {
    let entry_def = EntryDef::default_with_id("entrydef");

    InlineZomeSet::new_unique_single("simple", "integrity_simple", vec![entry_def.clone()])
        .callback("simple", "create", move |api, ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                ("integrity_simple", entry_def_id).into(),
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .callback("simple", "read", |api, hash: HeaderHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        })
}
