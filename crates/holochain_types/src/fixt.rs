//! Fixture definitions for crate structs

#![allow(missing_docs)]

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

// NB: Record is defined in holochain_zome_types, but I don't know if it's possible to define
//     new Curves on fixturators in other crates, so we have the definition in this crate so that
//     all Curves can be defined at once -MD
fixturator!(
    Record;
    vanilla fn record_with_no_entry(Signature, Action);
    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) | Entry::CounterSign(_, _) => EntryType::App(AppEntryDefFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
            Entry::Agent(_) => EntryType::AgentPubKey,
            Entry::CapClaim(_) => EntryType::CapClaim,
            Entry::CapGrant(_) => EntryType::CapGrant,
        };
        let mut action = ActionFixturator::new_indexed(CreateAction, get_fixt_index!()).next().unwrap();
        *action.entry_type_mut().unwrap() = et;
        let signature = SignatureFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        let shh = record_with_no_entry(signature, action).signed_action;
        Record::new(shh, RecordEntry::Present(get_fixt_curve!()))
    };
);
