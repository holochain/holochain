//! Collection of commonly used InlineZomes

use holo_hash::*;
use holochain_types::inline_zome::InlineEntryTypes;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::prelude::*;

/// An InlineZome with simple Create and Read operations
pub fn simple_create_read_zome() -> InlineZomeSet {
    InlineZomeSet::new_unique_single("simple", "integrity_simple", InlineEntryTypes::entry_defs())
        .callback("simple", "create", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, InlineEntryTypes::A),
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
