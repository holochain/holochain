use crate::prelude::*;

/// Create [`EntryHash`].
pub fn eh(i: u8) -> EntryHash {
    EntryHash::from_raw_36(vec![i; 36])
}

/// Create [`ActionHash`].
pub fn ah(i: u8) -> ActionHash {
    ActionHash::from_raw_36(vec![i; 36])
}

/// Create [`AgentPubKey`].
pub fn ak(i: u8) -> AgentPubKey {
    AgentPubKey::from_raw_36(vec![i; 36])
}

/// Create [`AnyLinkableHash`].
pub fn lh(i: u8) -> AnyLinkableHash {
    AnyLinkableHash::from(EntryHash::from_raw_36(vec![i; 36]))
}

/// Create [`DnaHash`].
pub fn dh(i: u8) -> DnaHash {
    DnaHash::from_raw_36(vec![i; 36])
}

/// Create [`Entry`].
pub fn e(e: impl TryInto<Entry>) -> Entry {
    match e.try_into() {
        Ok(e) => e,
        Err(_) => todo!(),
    }
}

/// Create public [`AppEntryDef`].
pub fn public_app_entry_def(zome_index: u8, entry_index: u8) -> AppEntryDef {
    AppEntryDef {
        entry_index: entry_index.into(),
        zome_index: zome_index.into(),
        visibility: EntryVisibility::Public,
    }
}

/// Create private [`AppEntryDef`].
pub fn private_app_entry_def(zome_index: u8, entry_index: u8) -> AppEntryDef {
    AppEntryDef {
        entry_index: entry_index.into(),
        zome_index: zome_index.into(),
        visibility: EntryVisibility::Private,
    }
}
