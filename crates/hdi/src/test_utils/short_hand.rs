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

/// Create [`Op::RegisterAgentActivity`].
pub fn r_activity(action: Action) -> Op {
    Op::RegisterAgentActivity(RegisterAgentActivity {
        action: SignedHashed {
            hashed: HoloHashed {
                content: action,
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
        cached_entry: None,
    })
}

/// Create [`Op::StoreRecord`].
pub fn s_record(action: Action, entry: RecordEntry) -> Op {
    Op::StoreRecord(StoreRecord {
        record: Record {
            signed_action: SignedHashed {
                hashed: HoloHashed {
                    content: action,
                    hash: ah(0),
                },
                signature: Signature([0u8; 64]),
            },
            entry,
        },
    })
}

/// Create [`Op::StoreEntry`].
pub fn s_entry(action: EntryCreationAction, entry: Entry) -> Op {
    Op::StoreEntry(StoreEntry {
        action: SignedHashed {
            hashed: HoloHashed {
                content: action,
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
        entry,
    })
}

/// Create [`Op::RegisterUpdate`].
pub fn r_update(
    original_action: EntryCreationAction,
    original_entry: Option<Entry>,
    update: Update,
    new_entry: Option<Entry>,
) -> Op {
    Op::RegisterUpdate(RegisterUpdate {
        original_action,
        original_entry,
        update: SignedHashed {
            hashed: HoloHashed {
                content: update,
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
        new_entry,
    })
}

/// Create [`Op::RegisterDelete`].
pub fn r_delete(original_entry_type: EntryType, original_entry: Option<Entry>) -> Op {
    Op::RegisterDelete(RegisterDelete {
        delete: SignedHashed {
            hashed: HoloHashed {
                content: Delete {
                    author: ak(0),
                    timestamp: Timestamp(0),
                    action_seq: 1,
                    prev_action: ah(0),
                    deletes_address: ah(2),
                    deletes_entry_address: eh(1),
                    weight: Default::default(),
                },
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
        original_action: EntryCreationAction::Create(c(original_entry_type)),
        original_entry,
    })
}

/// Create [`Op::RegisterCreateLink`].
pub fn r_create_link(zome_index: u8, link_type: u8) -> Op {
    Op::RegisterCreateLink(RegisterCreateLink {
        create_link: SignedHashed {
            hashed: HoloHashed {
                content: cl(zome_index, link_type),
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
    })
}

/// Create [`Op::RegisterDeleteLink`].
pub fn r_delete_link(zome_index: u8, link_type: u8) -> Op {
    Op::RegisterDeleteLink(RegisterDeleteLink {
        delete_link: SignedHashed {
            hashed: HoloHashed {
                content: DeleteLink {
                    author: ak(0),
                    timestamp: Timestamp(0),
                    action_seq: 1,
                    prev_action: ah(0),
                    base_address: eh(0).into(),
                    link_add_address: ah(2),
                },
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
        create_link: cl(zome_index, link_type),
    })
}

/// Create [`Create`].
pub fn c(entry_type: EntryType) -> Create {
    Create {
        author: ak(0),
        timestamp: Timestamp(0),
        action_seq: 1,
        prev_action: ah(0),
        entry_hash: eh(0),
        entry_type,
        weight: Default::default(),
    }
}

/// Create [`Update`].
pub fn u(entry_type: EntryType) -> Update {
    Update {
        author: ak(0),
        timestamp: Timestamp(0),
        action_seq: 1,
        prev_action: ah(0),
        entry_hash: eh(0),
        entry_type,
        weight: Default::default(),
        original_action_address: ah(1),
        original_entry_address: eh(1),
    }
}

/// Create [`CreateLink`].
pub fn cl(zome_index: u8, link_type: u8) -> CreateLink {
    CreateLink {
        author: ak(0),
        timestamp: Timestamp(0),
        action_seq: 1,
        prev_action: ah(0),
        zome_index: zome_index.into(),
        link_type: link_type.into(),
        weight: Default::default(),
        base_address: eh(0).into(),
        target_address: eh(1).into(),
        tag: ().into(),
    }
}

/// Create public app [`Action::Create`].
pub fn create_entry(zome_index: u8, entry_index: u8) -> Action {
    Action::Create(c(EntryType::App(AppEntryDef {
        entry_index: entry_index.into(),
        zome_index: zome_index.into(),
        visibility: EntryVisibility::Public,
    })))
}

/// Create private app [`Action::Create`].
pub fn create_hidden_entry(zome_index: u8, entry_index: u8) -> Action {
    Action::Create(c(EntryType::App(AppEntryDef {
        entry_index: entry_index.into(),
        zome_index: zome_index.into(),
        visibility: EntryVisibility::Private,
    })))
}

/// Create [`Action::CreateLink`].
pub fn create_link(z: u8, lt: u8) -> Action {
    Action::CreateLink(cl(z, lt))
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
