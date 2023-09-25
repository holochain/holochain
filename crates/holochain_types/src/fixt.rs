//! Fixture definitions for crate structs

#![allow(missing_docs)]

use crate::action::NewEntryAction;
use crate::prelude::*;
use ::fixt::prelude::*;
use rand::seq::IteratorRandom;
use std::iter::Iterator;

pub use holochain_zome_types::fixt::*;

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
            let (shh, _) = record.into_inner();
            Record::new(shh, Some(entry))
        }
        ActionType::Update => {
            let u = UpdateFixturator::new_indexed(et, index).next().unwrap();
            let u = NewEntryAction::Update(u);
            let record: Record = RecordFixturator::new_indexed(u, index).next().unwrap();
            let (shh, _) = record.into_inner();
            Record::new(shh, Some(entry))
        }
        _ => panic!("You choose {:?} for a Record with en Entry", action_type),
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
        record_with_no_entry(s, get_fixt_curve!().into())
    };
    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) | Entry::CounterSign(_, _) => EntryType::App(AppEntryDefFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
            Entry::Agent(_) => EntryType::AgentPubKey,
            Entry::CapClaim(_) => EntryType::CapClaim,
            Entry::CapGrant(_) => EntryType::CapGrant,
        };
        let new = NewEntryActionFixturator::new_indexed(et, get_fixt_index!()).next().unwrap();
        let (shh, _) = RecordFixturator::new_indexed(new, get_fixt_index!()).next().unwrap().into_inner();
        Record::new(shh, Some(get_fixt_curve!()))
    };
    curve NewEntryRecord {
        new_entry_record(get_fixt_curve!().0, get_fixt_curve!().1, get_fixt_index!())
    };
);
