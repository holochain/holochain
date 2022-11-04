//! Collection of commonly used InlineZomes

use holo_hash::*;
use holochain_types::inline_zome::InlineEntryTypes;
use holochain_types::inline_zome::InlineZomeSet;
use holochain_zome_types::prelude::*;

use crate::sweettest::SweetInlineZomes;

/// An InlineZome with simple Create and Read operations
pub fn simple_create_read_zome() -> InlineIntegrityZome {
    InlineIntegrityZome::new_unique(InlineEntryTypes::entry_defs(), 0)
        .function("create", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, InlineEntryTypes::A),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("read", |api, hash: ActionHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        })
}

/// An InlineZome with a function to create many random entries at once,
/// and a function to read the entry at a hash
pub fn batch_create_zome() -> InlineIntegrityZome {
    use rand::Rng;

    #[derive(Copy, Clone, Debug, Serialize, Deserialize, SerializedBytes)]
    struct RandNum(u64);

    impl RandNum {
        pub fn new() -> Self {
            Self(rand::thread_rng().gen())
        }
    }

    InlineIntegrityZome::new_unique(InlineEntryTypes::entry_defs(), 0)
        .function("create_batch", move |api, num: usize| {
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
        .function("read", |api, hash: ActionHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        })
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
    SerializedBytes,
    derive_more::From,
)]
#[serde(transparent)]
#[repr(transparent)]
/// Newtype for simple_crud_zome entry type
pub struct AppString(pub String);

impl AppString {
    /// Constructor
    pub fn new<S: Into<String>>(s: S) -> Self {
        AppString(s.into())
    }
}

/// An InlineZome with simple Create and Read operations
pub fn simple_crud_zome() -> InlineZomeSet {
    let string_entry_def = EntryDef::from_id("string");
    let unit_entry_def = EntryDef::from_id("unit");
    let bytes_entry_def = EntryDef::from_id("bytes");

    SweetInlineZomes::new(vec![string_entry_def, unit_entry_def, bytes_entry_def], 0)
        .function("create_string", move |api, s: AppString| {
            let entry = Entry::app(s.try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("create_unit", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(1)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("create_bytes", move |api, bs: Vec<u8>| {
            let entry = Entry::app(UnsafeBytes::try_from(bs).unwrap().into()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(2)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        })
        .function("delete", move |api, action_hash: ActionHash| {
            let hash = api.delete(DeleteInput::new(action_hash, ChainTopOrdering::default()))?;
            Ok(hash)
        })
        .function("read", |api, hash: ActionHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map(|e| e.into_iter().next().unwrap())
                .map_err(Into::into)
        })
        .function("read_multi", |api, hashes: Vec<ActionHash>| {
            let gets = hashes
                .iter()
                .map(|h| GetInput::new(h.clone().into(), GetOptions::default()))
                .collect();
            api.get(gets).map_err(Into::into)
        })
        .function("read_entry", |api, hash: EntryHash| {
            api.get(vec![GetInput::new(hash.into(), GetOptions::default())])
                .map_err(Into::into)
        })
        .function("emit_signal", |api, ()| {
            api.emit_signal(AppSignal::new(ExternIO::encode(()).unwrap()))
                .map_err(Into::into)
        })
        .0
}
