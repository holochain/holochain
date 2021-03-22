//! Fixture definitions for crate structs

#![allow(missing_docs)]

use crate::dna::zome::HostFnAccess;
use crate::dna::zome::Permission;
use crate::dna::zome::Zome;
use crate::dna::zome::ZomeDef;
use crate::dna::DnaDef;
use crate::dna::Zomes;
use crate::header::NewEntryHeader;
use ::fixt::prelude::*;
use holochain_zome_types::prelude::*;
use rand::seq::IteratorRandom;
use std::iter::Iterator;

pub use holochain_zome_types::fixt::*;

fixturator!(
    Permission;
    unit variants [ Allow Deny ] empty Deny;
);

fixturator!(
    HostFnAccess;
    constructor fn new(Permission, Permission, Permission, Permission, Permission, Permission, Permission);
);

fixturator!(
    CellId;
    constructor fn new(DnaHash, AgentPubKey);
);

fixturator!(
    Zome;
    constructor fn new(ZomeName, ZomeDef);
);

fixturator!(
    Zomes;
    curve Empty Vec::new();
    curve Unpredictable {
        // @todo implement unpredictable zomes
        ZomesFixturator::new(Empty).next().unwrap()
    };
    curve Predictable {
        // @todo implement predictable zomes
        ZomesFixturator::new(Empty).next().unwrap()
    };
);

fixturator!(
    ZomeDef;
    constructor fn from_hash(WasmHash);
);

fixturator!(
    NewEntryHeader;
    variants [
        Create(Create)
        Update(Update)
    ];


    curve PublicCurve {
        match fixt!(NewEntryHeader) {
            NewEntryHeader::Create(_) => NewEntryHeader::Create(fixt!(Create, PublicCurve)),
            NewEntryHeader::Update(_) => NewEntryHeader::Update(fixt!(Update, PublicCurve)),
        }
    };

    curve EntryType {
        match fixt!(NewEntryHeader) {
            NewEntryHeader::Create(_) => {
                let ec = CreateFixturator::new_indexed(get_fixt_curve!(), get_fixt_index!()).next().unwrap();
                NewEntryHeader::Create(ec)
            },
            NewEntryHeader::Update(_) => {
                let eu = UpdateFixturator::new_indexed(get_fixt_curve!(), get_fixt_index!()).next().unwrap();
                NewEntryHeader::Update(eu)
            },
        }
    };
);

fn new_entry_element(entry: Entry, header_type: HeaderType, index: usize) -> Element {
    let et = match entry {
        Entry::App(_) => EntryType::App(
            AppEntryTypeFixturator::new_indexed(Unpredictable, index)
                .next()
                .unwrap(),
        ),
        Entry::Agent(_) => EntryType::AgentPubKey,
        Entry::CapClaim(_) => EntryType::CapClaim,
        Entry::CapGrant(_) => EntryType::CapGrant,
    };
    match header_type {
        HeaderType::Create => {
            let c = CreateFixturator::new_indexed(et, index).next().unwrap();
            let c = NewEntryHeader::Create(c);
            let element: Element = ElementFixturator::new_indexed(c, index).next().unwrap();
            let (shh, _) = element.into_inner();
            Element::new(shh, Some(entry))
        }
        HeaderType::Update => {
            let u = UpdateFixturator::new_indexed(et, index).next().unwrap();
            let u = NewEntryHeader::Update(u);
            let element: Element = ElementFixturator::new_indexed(u, index).next().unwrap();
            let (shh, _) = element.into_inner();
            Element::new(shh, Some(entry))
        }
        _ => panic!("You choose {:?} for an Element with en Entry", header_type),
    }
}

type NewEntryElement = (Entry, HeaderType);

// NB: Element is defined in holochain_zome_types, but I don't know if it's possible to define
//     new Curves on fixturators in other crates, so we have the definition in this crate so that
//     all Curves can be defined at once -MD
fixturator!(
    Element;
    vanilla fn element_with_no_entry(Signature, Header);
    curve NewEntryHeader {
        let s = SignatureFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        element_with_no_entry(s, get_fixt_curve!().into())
    };
    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) => EntryType::App(AppEntryTypeFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
            Entry::Agent(_) => EntryType::AgentPubKey,
            Entry::CapClaim(_) => EntryType::CapClaim,
            Entry::CapGrant(_) => EntryType::CapGrant,
        };
        let new = NewEntryHeaderFixturator::new_indexed(et, get_fixt_index!()).next().unwrap();
        let (shh, _) = ElementFixturator::new_indexed(new, get_fixt_index!()).next().unwrap().into_inner();
        Element::new(shh, Some(get_fixt_curve!()))
    };
    curve NewEntryElement {
        new_entry_element(get_fixt_curve!().0, get_fixt_curve!().1, get_fixt_index!())
    };
);

fixturator!(
    ValidateData;
    constructor fn new_element_only (Element);
);

fixturator!(
    DnaDef;
    curve Empty DnaDef {
        name: StringFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
    };

    curve Unpredictable DnaDef {
        name: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
    };

    curve Predictable DnaDef {
        name: StringFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
    };
);
