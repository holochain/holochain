//! Collection of commonly used InlineZomes

use holo_hash::*;
use holochain_zome_types::prelude::*;

/// An InlineZome with simple Create and Read operations
pub fn simple_create_read_zome() -> InlineZome {
    let entry_def = EntryDef::default_with_id("entrydef");

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create", move |api, ()| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                entry_def_id,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        })
}

/// An InlineZome with a function to create many random entries at once,
/// and a function to read the entry at a hash
pub fn batch_create_zome() -> InlineZome {
    use rand::Rng;

    let entry_def = EntryDef::default_with_id("entrydef");

    #[derive(Copy, Clone, Debug, Serialize, Deserialize, SerializedBytes)]
    struct RandNum(u64);

    impl RandNum {
        pub fn new() -> Self {
            Self(rand::thread_rng().gen())
        }
    }

    InlineZome::new_unique(vec![entry_def.clone()])
        .callback("create_batch", move |api, num: usize| {
            let entry_def_id: EntryDefId = entry_def.id.clone();
            let hashes = std::iter::repeat_with(|| {
                api.create(CreateInput::new(
                    entry_def_id.clone(),
                    Entry::app(RandNum::new().try_into().unwrap()).unwrap(),
                    ChainTopOrdering::default(),
                ))
                .unwrap()
            })
            .take(num)
            .collect::<Vec<_>>();
            Ok(hashes)
        })
        .callback("read", |api, hash: HeaderHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        })
}
