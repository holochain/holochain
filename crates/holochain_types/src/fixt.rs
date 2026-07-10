//! Fixture definitions for crate structs

#![allow(missing_docs)]

use crate::action::NewEntryAction;
use crate::prelude::*;
use ::fixt::prelude::*;
pub use holochain_zome_types::fixt::*;
use rand::seq::IteratorRandom;
use std::iter::Iterator;

fixturator!(
    Permission;
    unit variants [ Allow Deny ] empty Deny;
);

fixturator!(
    HostFnAccess;
    constructor fn new(Permission, Permission, Permission, Permission, Permission, Permission, Permission, Permission, Permission, Permission);
);

fixturator!(
    NewEntryAction;
    variants [
        Create(Create)
        Update(Update)
    ];


    curve PublicCurve {
        match fixt!(NewEntryAction) {
            NewEntryAction::Create(_) => NewEntryAction::Create(fixt!(Create, PublicCurve)),
            NewEntryAction::Update(_) => NewEntryAction::Update(fixt!(Update, PublicCurve)),
        }
    };

    curve EntryType {
        match fixt!(NewEntryAction) {
            NewEntryAction::Create(_) => {
                let ec = CreateFixturator::new_indexed(get_fixt_curve!(), get_fixt_index!()).next().unwrap();
                NewEntryAction::Create(ec)
            },
            NewEntryAction::Update(_) => {
                let eu = UpdateFixturator::new_indexed(get_fixt_curve!(), get_fixt_index!()).next().unwrap();
                NewEntryAction::Update(eu)
            },
        }
    };
);

fn new_entry_record(entry: Entry, action_type: ActionType, index: usize) -> Record {
    let et = match entry {
        Entry::App(_) | Entry::CounterSign(_, _) => EntryType::App(
            AppEntryDefFixturator::new_indexed(Unpredictable, index)
                .next()
                .unwrap(),
        ),
        Entry::Agent(_) => EntryType::AgentPubKey,
        Entry::CapClaim(_) => EntryType::CapClaim,
        Entry::CapGrant(_) => EntryType::CapGrant,
    };
    match action_type {
        ActionType::Create => {
            let c = CreateFixturator::new_indexed(et, index).next().unwrap();
            let c = NewEntryAction::Create(c);
            let record: Record = RecordFixturator::new_indexed(c, index).next().unwrap();
            Record::new(record.signed_action, RecordEntry::Present(entry))
        }
        ActionType::Update => {
            let u = UpdateFixturator::new_indexed(et, index).next().unwrap();
            let u = NewEntryAction::Update(u);
            let record: Record = RecordFixturator::new_indexed(u, index).next().unwrap();
            Record::new(record.signed_action, RecordEntry::Present(entry))
        }
        _ => panic!("You choose {action_type:?} for a Record with en Entry"),
    }
}

/// Project a legacy [`NewEntryAction`] (a `Create` or `Update` struct) onto the
/// v2 [`Action`] shape, building [`ActionData`] directly. Seeds fixtures only;
/// production authoring builds v2 actions natively.
fn new_entry_action_to_v2(new_entry_action: NewEntryAction) -> Action {
    use holochain_zome_types::dht_v2::{ActionHeader, CreateData, UpdateData};
    match new_entry_action {
        NewEntryAction::Create(c) => Action {
            header: ActionHeader {
                author: c.author,
                timestamp: c.timestamp,
                action_seq: c.action_seq,
                prev_action: Some(c.prev_action),
            },
            data: ActionData::Create(CreateData {
                entry_type: c.entry_type,
                entry_hash: c.entry_hash,
            }),
        },
        NewEntryAction::Update(u) => Action {
            header: ActionHeader {
                author: u.author,
                timestamp: u.timestamp,
                action_seq: u.action_seq,
                prev_action: Some(u.prev_action),
            },
            data: ActionData::Update(UpdateData {
                original_action_address: u.original_action_address,
                original_entry_address: u.original_entry_address,
                entry_type: u.entry_type,
                entry_hash: u.entry_hash,
            }),
        },
    }
}

type NewEntryRecord = (Entry, ActionType);

// NB: Record is defined in holochain_zome_types, but I don't know if it's possible to define
//     new Curves on fixturators in other crates, so we have the definition in this crate so that
//     all Curves can be defined at once -MD
fixturator!(
    Record;
    vanilla fn record_with_no_entry(Signature, Action);
    curve NewEntryAction {
        let s = SignatureFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        record_with_no_entry(s, new_entry_action_to_v2(get_fixt_curve!()))
    };
    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) | Entry::CounterSign(_, _) => EntryType::App(AppEntryDefFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
            Entry::Agent(_) => EntryType::AgentPubKey,
            Entry::CapClaim(_) => EntryType::CapClaim,
            Entry::CapGrant(_) => EntryType::CapGrant,
        };
        let new = NewEntryActionFixturator::new_indexed(et, get_fixt_index!()).next().unwrap();
        let shh = RecordFixturator::new_indexed(new, get_fixt_index!()).next().unwrap().signed_action;
        Record::new(shh, RecordEntry::Present(get_fixt_curve!()))
    };
    curve NewEntryRecord {
        new_entry_record(get_fixt_curve!().0, get_fixt_curve!().1, get_fixt_index!())
    };
);
