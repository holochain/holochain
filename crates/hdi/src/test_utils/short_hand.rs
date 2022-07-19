use crate::prelude::*;

pub fn eh(i: u8) -> EntryHash {
    EntryHash::from_raw_36(vec![i; 36])
}

pub fn ah(i: u8) -> ActionHash {
    ActionHash::from_raw_36(vec![i; 36])
}

pub fn ak(i: u8) -> AgentPubKey {
    AgentPubKey::from_raw_36(vec![i; 36])
}

pub fn lh(i: u8) -> AnyLinkableHash {
    AnyLinkableHash::from(EntryHash::from_raw_36(vec![i; 36]))
}

pub fn dh(i: u8) -> DnaHash {
    DnaHash::from_raw_36(vec![i; 36])
}

pub fn activity(action: Action) -> Op {
    Op::RegisterAgentActivity(RegisterAgentActivity {
        action: SignedHashed {
            hashed: HoloHashed {
                content: action,
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
    })
}

pub fn record(action: Action, entry: RecordEntry) -> Op {
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

pub fn entry(action: EntryCreationAction, entry: Entry) -> Op {
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

pub fn r_create_link(zome_id: u8, link_type: u8) -> Op {
    Op::RegisterCreateLink(RegisterCreateLink {
        create_link: SignedHashed {
            hashed: HoloHashed {
                content: cl(zome_id, link_type),
                hash: ah(0),
            },
            signature: Signature([0u8; 64]),
        },
    })
}

pub fn r_delete_link(zome_id: u8, link_type: u8) -> Op {
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
        create_link: cl(zome_id, link_type),
    })
}

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

pub fn cl(zome_id: u8, link_type: u8) -> CreateLink {
    CreateLink {
        author: ak(0),
        timestamp: Timestamp(0),
        action_seq: 1,
        prev_action: ah(0),
        zome_id: zome_id.into(),
        link_type: link_type.into(),
        weight: Default::default(),
        base_address: eh(0).into(),
        target_address: eh(1).into(),
        tag: ().into(),
    }
}

pub fn create_entry(z: u8, et: u8) -> Action {
    Action::Create(c(EntryType::App(AppEntryType {
        id: et.into(),
        zome_id: z.into(),
        visibility: EntryVisibility::Public,
    })))
}

pub fn create_hidden_entry(z: u8, et: u8) -> Action {
    Action::Create(c(EntryType::App(AppEntryType {
        id: et.into(),
        zome_id: z.into(),
        visibility: EntryVisibility::Private,
    })))
}

pub fn create_link(z: u8, lt: u8) -> Action {
    Action::CreateLink(cl(z, lt))
}

pub fn e(e: impl TryInto<Entry>) -> Entry {
    match e.try_into() {
        Ok(e) => e,
        Err(_) => todo!(),
    }
}

pub fn public_aet(z: u8, et: u8) -> AppEntryType {
    AppEntryType {
        id: et.into(),
        zome_id: z.into(),
        visibility: EntryVisibility::Public,
    }
}

pub fn private_aet(z: u8, et: u8) -> AppEntryType {
    AppEntryType {
        id: et.into(),
        zome_id: z.into(),
        visibility: EntryVisibility::Private,
    }
}
