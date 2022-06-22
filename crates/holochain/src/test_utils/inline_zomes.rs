//! Collection of commonly used InlineZomes

use holo_hash::*;
use holochain_types::inline_zome::InlineEntryTypes;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::prelude::*;

use crate::sweettest::SweetEasyInline;

/// An InlineZome with simple Create and Read operations
pub fn simple_create_read_zome() -> InlineZomeSet {
    InlineZomeSet::new_unique_single(
        "simple",
        "integrity_simple",
        InlineEntryTypes::entry_defs(),
        0,
    )
    .callback("simple", "create", move |api, ()| {
        let entry = Entry::app(().try_into().unwrap()).unwrap();
        let hash = api.create(CreateInput::new(
            InlineZomeSet::get_entry_location(&api, InlineEntryTypes::A),
            EntryVisibility::Public,
            entry,
            ChainTopOrdering::default(),
        ))?;
        Ok(hash)
    })
    .callback("simple", "read", |api, hash: ActionHash| {
        api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
            .map(|e| e.into_iter().next().unwrap())
            .map_err(Into::into)
    })
}

/// An InlineZome with a function to create many random entries at once,
/// and a function to read the entry at a hash
pub fn batch_create_zome() -> InlineZomeSet {
    use rand::Rng;

    #[derive(Copy, Clone, Debug, Serialize, Deserialize, SerializedBytes)]
    struct RandNum(u64);

    impl RandNum {
        pub fn new() -> Self {
            Self(rand::thread_rng().gen())
        }
    }

    SweetEasyInline::new(InlineEntryTypes::entry_defs(), 0)
        .callback("create_batch", move |api, num: usize| {
            let hashes = std::iter::repeat_with(|| {
                api.create(CreateInput::new(
                    InlineZomeSet::get_entry_location(&api, InlineEntryTypes::A),
                    EntryVisibility::Public,
                    Entry::app(RandNum::new().try_into().unwrap()).unwrap(),
                    ChainTopOrdering::default(),
                ))
                .unwrap()
            })
            .take(num)
            .collect::<Vec<_>>();
            Ok(hashes)
        })
        .callback("read", |api, hash: ActionHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        })
        .0
}
