//! Fixture definitions for holochain_types structs

// FIXME (aka fixtme, haha, get it?) move other fixturators from this crate into this module

#![allow(missing_docs)]

use crate::cell::CellId;
use crate::dna::zome::Zome;
use crate::dna::zome::{HostFnAccess, Permission};
use crate::dna::DnaDef;
use crate::dna::Zomes;
use crate::header::NewEntryHeader;
use crate::Timestamp;
use ::fixt::prelude::*;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::capability::CapGrant;
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::capability::CurryPayloads;
use holochain_zome_types::capability::GrantedFunction;
use holochain_zome_types::capability::GrantedFunctions;
use holochain_zome_types::capability::ZomeCallCapGrant;
use holochain_zome_types::capability::CAP_SECRET_BYTES;
use holochain_zome_types::crdt::CrdtType;
use holochain_zome_types::entry::AppEntryBytes;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::entry_def::EntryDefId;
use holochain_zome_types::entry_def::EntryDefs;
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::entry_def::RequiredValidations;
use holochain_zome_types::header::AgentValidationPkg;
use holochain_zome_types::header::AppEntryType;
use holochain_zome_types::header::CloseChain;
use holochain_zome_types::header::Create;
use holochain_zome_types::header::Delete;
use holochain_zome_types::header::Dna;
use holochain_zome_types::header::EntryType;
use holochain_zome_types::header::Header;
use holochain_zome_types::header::InitZomesComplete;
use holochain_zome_types::header::OpenChain;
use holochain_zome_types::header::Update;
use holochain_zome_types::header::ZomeId;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::signature::Signature;
use holochain_zome_types::zome::FunctionName;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::Entry;
use holochain_zome_types::{
    capability::CapAccess, element::Element, element::SignedHeaderHashed, header::HeaderHashed,
};
use holochain_zome_types::{capability::CapClaim, header::HeaderType};
use rand::seq::IteratorRandom;
use rand::Rng;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::iter::Iterator;

pub use holochain_zome_types::fixt::{TimestampFixturator as ZomeTimestampFixturator, *};

/// a curve to spit out Entry::App values
#[derive(Clone)]
pub struct AppEntry;

/// A curve to make headers have public entry types
#[derive(Clone)]
pub struct PublicCurve;

fixturator!(
    Zome;
    constructor fn from_hash(WasmHash);
);

fixturator!(
    ZomeName;
    from String;
);

fixturator!(
    FunctionName;
    from String;
);

fixturator!(
    CapSecret;
    curve Empty [0; CAP_SECRET_BYTES].into();
    curve Unpredictable {
        let mut rng = rand::thread_rng();
        let upper = rng.gen::<[u8; CAP_SECRET_BYTES / 2]>();
        let lower = rng.gen::<[u8; CAP_SECRET_BYTES / 2]>();
        let mut inner = [0; CAP_SECRET_BYTES];
        inner[..CAP_SECRET_BYTES / 2].copy_from_slice(&lower);
        inner[CAP_SECRET_BYTES / 2..].copy_from_slice(&upper);
        inner.into()
    };
    curve Predictable [get_fixt_index!() as u8; CAP_SECRET_BYTES].into();
);

fixturator!(
    ZomeId;
    from u8;
);

fixturator!(
    CapClaim;
    constructor fn new(String, AgentPubKey, CapSecret);
);

fixturator!(
    Timestamp;
    constructor fn now();
);

newtype_fixturator!(Signature<Bytes>);

fixturator!(
    MigrateAgent;
    unit variants [ Open Close ] empty Close;
);

fixturator!(
    GrantedFunction;
    curve Empty (
        ZomeNameFixturator::new_indexed(Empty, get_fixt_index!()).next().unwrap(),
        FunctionNameFixturator::new_indexed(Empty, get_fixt_index!()).next().unwrap()
    );
    curve Unpredictable (
        ZomeNameFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap(),
        FunctionNameFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()
    );
    curve Predictable (
        ZomeNameFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap(),
        FunctionNameFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()
    );
);

fixturator!(
    CurryPayloads;
    curve Empty CurryPayloads(BTreeMap::new());
    curve Unpredictable {
        let mut rng = rand::thread_rng();
        let number_of_payloads = rng.gen_range(0, 5);

        let mut payloads: BTreeMap<GrantedFunction, SerializedBytes> = BTreeMap::new();
        let mut granted_function_fixturator = GrantedFunctionFixturator::new_indexed(Unpredictable, get_fixt_index!());
        let mut sb_fixturator = SerializedBytesFixturator::new_indexed(Unpredictable, get_fixt_index!());
        for _ in 0..number_of_payloads {
            payloads.insert(granted_function_fixturator.next().unwrap(), sb_fixturator.next().unwrap());
        }
        CurryPayloads(payloads)
    };
    curve Predictable {
        let mut rng = rand::thread_rng();
        let number_of_payloads = rng.gen_range(0, 5);

        let mut payloads: BTreeMap<GrantedFunction, SerializedBytes> = BTreeMap::new();
        let mut granted_function_fixturator = GrantedFunctionFixturator::new_indexed(Predictable, get_fixt_index!());
        let mut sb_fixturator = SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!());
        for _ in 0..number_of_payloads {
            payloads.insert(granted_function_fixturator.next().unwrap(), sb_fixturator.next().unwrap());
        }
        CurryPayloads(payloads)
    };
);

fixturator!(
    ZomeCallCapGrant,
    {
        ZomeCallCapGrant::new(
            StringFixturator::new(Empty).next().unwrap(),
            CapAccessFixturator::new(Empty).next().unwrap(),
            {
                let mut rng = rand::thread_rng();
                let number_of_zomes = rng.gen_range(0, 5);

                let mut granted_functions: GrantedFunctions = HashSet::new();
                for _ in 0..number_of_zomes {
                    granted_functions.insert(GrantedFunctionFixturator::new(Empty).next().unwrap());
                }
                granted_functions
            }, // CurryPayloadsFixturator::new(Empty).next().unwrap(),
        )
    },
    {
        ZomeCallCapGrant::new(
            StringFixturator::new(Unpredictable).next().unwrap(),
            CapAccessFixturator::new(Unpredictable).next().unwrap(),
            {
                let mut rng = rand::thread_rng();
                let number_of_zomes = rng.gen_range(0, 5);

                let mut granted_functions: GrantedFunctions = HashSet::new();
                for _ in 0..number_of_zomes {
                    granted_functions.insert(
                        GrantedFunctionFixturator::new(Unpredictable)
                            .next()
                            .unwrap(),
                    );
                }
                granted_functions
            },
            // CurryPayloadsFixturator::new(Unpredictable).next().unwrap(),
        )
    },
    {
        ZomeCallCapGrant::new(
            StringFixturator::new_indexed(Predictable, get_fixt_index!())
                .next()
                .unwrap(),
            CapAccessFixturator::new_indexed(Predictable, get_fixt_index!())
                .next()
                .unwrap(),
            {
                let mut granted_functions: GrantedFunctions = HashSet::new();
                for _ in 0..get_fixt_index!() % 3 {
                    granted_functions
                        .insert(GrantedFunctionFixturator::new(Predictable).next().unwrap());
                }
                granted_functions
            },
            // CurryPayloadsFixturator::new(Predictable).next().unwrap(),
        )
    }
);

fixturator!(
    CapAccess;

    enum [ Unrestricted Transferable Assigned ];

    curve Empty {
        match CapAccessVariant::random() {
            CapAccessVariant::Unrestricted => CapAccess::from(()),
            CapAccessVariant::Transferable => CapAccess::from(CapSecretFixturator::new_indexed(Empty, get_fixt_index!()).next().unwrap()),
            CapAccessVariant::Assigned => CapAccess::from((
                CapSecretFixturator::new_indexed(Empty, get_fixt_index!()).next().unwrap(),
                HashSet::new()
            ))
        }
    };

    curve Unpredictable {
        match CapAccessVariant::random() {
            CapAccessVariant::Unrestricted => CapAccess::from(()),
            CapAccessVariant::Transferable => {
                CapAccess::from(CapSecretFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap())
            },
            CapAccessVariant::Assigned => {
                let mut rng = rand::thread_rng();
                let number_of_assigned = rng.gen_range(0, 5);

                CapAccess::from((
                    CapSecretFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap(),
                    {
                        let mut set: HashSet<AgentPubKey> = HashSet::new();
                        for _ in 0..number_of_assigned {
                            set.insert(AgentPubKeyFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap());
                        }
                        set
                    }
                ))
            }
        }
    };

    curve Predictable {
        match CapAccessVariant::nth(get_fixt_index!()) {
            CapAccessVariant::Unrestricted => CapAccess::from(()),
            CapAccessVariant::Transferable => CapAccess::from(CapSecretFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()),
            CapAccessVariant::Assigned => CapAccess::from((
                CapSecretFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap(),
            {
                let mut set: HashSet<AgentPubKey> = HashSet::new();
                for _ in 0..get_fixt_index!() % 3 {
                    set.insert(AgentPubKeyFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap());
                }
                set
            }))
        }
    };
);

fixturator!(
    CapGrant;
    variants [ ChainAuthor(AgentPubKey) RemoteAgent(ZomeCallCapGrant) ];
);

fn element_with_no_entry(signature: Signature, header: Header) -> Element {
    let shh =
        SignedHeaderHashed::with_presigned(HeaderHashed::from_content_sync(header), signature);
    Element::new(shh, None)
}

type NewEntryElement = (Entry, HeaderType);

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
        Element::new(shh, Some(get_fixt_curve!().clone()))
    };
    curve NewEntryElement {
        new_entry_element(get_fixt_curve!().0.clone(), get_fixt_curve!().1.clone(), get_fixt_index!())
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

fixturator!(
    Entry;
    variants [
        Agent(AgentPubKey)
        App(AppEntryBytes)
        CapClaim(CapClaim)
        CapGrant(ZomeCallCapGrant)
    ];

    curve AppEntry {
        Entry::App(
            AppEntryBytesFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()
        )
    };
);
use std::convert::TryFrom;

fixturator!(
    AppEntryBytes;
    curve Empty AppEntryBytes::try_from(
        SerializedBytesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap()
        ).unwrap();

    curve Predictable AppEntryBytes::try_from(
        SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap()
        ).unwrap();

    curve Unpredictable AppEntryBytes::try_from(
        SerializedBytesFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap()
        ).unwrap();
);

fixturator!(
    CrdtType;
    curve Empty CrdtType;
    curve Unpredictable CrdtType;
    curve Predictable CrdtType;
);

fixturator!(
    EntryDefId;
    from String;
);

fixturator!(
    RequiredValidations;
    from u8;
);

fixturator!(
    EntryDef;
    constructor fn new(EntryDefId, EntryVisibility, CrdtType, RequiredValidations, RequiredValidationType);
);

fixturator!(
    EntryDefs;
    curve Empty Vec::new().into();
    curve Unpredictable {
        let mut rng = rand::thread_rng();
        let number_of_defs = rng.gen_range(0, 5);

        let mut defs = vec![];
        let mut entry_def_fixturator = EntryDefFixturator::new(Unpredictable);
        for _ in 0..number_of_defs {
            defs.push(entry_def_fixturator.next().unwrap());
        }
        defs.into()
    };
    curve Predictable {
        let mut defs = vec![];
        let mut entry_def_fixturator = EntryDefFixturator::new(Predictable);
        for _ in 0..3 {
            defs.push(entry_def_fixturator.next().unwrap());
        }
        defs.into()
    };
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

fixturator!(
    Dna;
    constructor fn from_builder(DnaHash, HeaderBuilderCommon);
);

fixturator! {
    MaybeSerializedBytes;
    enum [ Some None ];
    curve Empty MaybeSerializedBytes::None;
    curve Unpredictable match MaybeSerializedBytesVariant::random() {
        MaybeSerializedBytesVariant::None => MaybeSerializedBytes::None,
        MaybeSerializedBytesVariant::Some => MaybeSerializedBytes::Some(fixt!(SerializedBytes)),
    };
    curve Predictable match MaybeSerializedBytesVariant::nth(get_fixt_index!()) {
        MaybeSerializedBytesVariant::None => MaybeSerializedBytes::None,
        MaybeSerializedBytesVariant::Some => MaybeSerializedBytes::Some(SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()),
    };
}

fixturator! {
    EntryType;
    enum [ AgentPubKey App CapClaim CapGrant ];
    curve Empty EntryType::AgentPubKey;
    curve Unpredictable match EntryTypeVariant::random() {
        EntryTypeVariant::AgentPubKey => EntryType::AgentPubKey,
        EntryTypeVariant::App => EntryType::App(fixt!(AppEntryType)),
        EntryTypeVariant::CapClaim => EntryType::CapClaim,
        EntryTypeVariant::CapGrant => EntryType::CapGrant,
    };
    curve Predictable match EntryTypeVariant::nth(get_fixt_index!()) {
        EntryTypeVariant::AgentPubKey => EntryType::AgentPubKey,
        EntryTypeVariant::App => EntryType::App(AppEntryTypeFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()),
        EntryTypeVariant::CapClaim => EntryType::CapClaim,
        EntryTypeVariant::CapGrant => EntryType::CapGrant,
    };
    curve PublicCurve {
        let aet = fixt!(AppEntryType);
        EntryType::App(AppEntryType::new(aet.id(), aet.zome_id(), EntryVisibility::Public))
    };
}

fixturator!(
    AgentValidationPkg;
    constructor fn from_builder(HeaderBuilderCommon, MaybeSerializedBytes);
);

fixturator!(
    InitZomesComplete;
    constructor fn from_builder(HeaderBuilderCommon);
);

fixturator!(
    OpenChain;
    constructor fn from_builder(HeaderBuilderCommon, DnaHash);
);

fixturator!(
    CloseChain;
    constructor fn from_builder(HeaderBuilderCommon, DnaHash);
);

fixturator!(
    Create;
    constructor fn from_builder(HeaderBuilderCommon, EntryType, EntryHash);

    curve PublicCurve {
        let mut ec = fixt!(Create);
        ec.entry_type = fixt!(EntryType, PublicCurve);
        ec
    };
    curve EntryType {
        let mut ec = CreateFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        ec.entry_type = get_fixt_curve!().clone();
        ec
    };
    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) => EntryType::App(AppEntryTypeFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
            Entry::Agent(_) => EntryType::AgentPubKey,
            Entry::CapClaim(_) => EntryType::CapClaim,
            Entry::CapGrant(_) => EntryType::CapGrant,
        };
        CreateFixturator::new_indexed(et, get_fixt_index!()).next().unwrap()
    };
);

type EntryTypeEntryHash = (EntryType, EntryHash);

fixturator!(
    Update;
    constructor fn from_builder(HeaderBuilderCommon, EntryHash, HeaderHash, EntryType, EntryHash);

    curve PublicCurve {
        let mut eu = fixt!(Update);
        eu.entry_type = fixt!(EntryType, PublicCurve);
        eu
    };

    curve EntryType {
        let mut eu = UpdateFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        eu.entry_type = get_fixt_curve!().clone();
        eu
    };

    curve EntryTypeEntryHash {
        let mut u = UpdateFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        u.entry_type = get_fixt_curve!().0.clone();
        u.entry_hash = get_fixt_curve!().1.clone();
        u
    };

    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) => EntryType::App(AppEntryTypeFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
            Entry::Agent(_) => EntryType::AgentPubKey,
            Entry::CapClaim(_) => EntryType::CapClaim,
            Entry::CapGrant(_) => EntryType::CapGrant,
        };
        let eh = EntryHash::with_data_sync(&get_fixt_curve!());
        UpdateFixturator::new_indexed((et, eh), get_fixt_index!()).next().unwrap()
    };
);

fixturator!(
    Delete;
    constructor fn from_builder(HeaderBuilderCommon, HeaderHash, EntryHash);
);

fixturator!(
    Header;
    variants [
        Dna(Dna)
        AgentValidationPkg(AgentValidationPkg)
        InitZomesComplete(InitZomesComplete)
        CreateLink(CreateLink)
        DeleteLink(DeleteLink)
        OpenChain(OpenChain)
        CloseChain(CloseChain)
        Create(Create)
        Update(Update)
        Delete(Delete)
    ];

    curve PublicCurve {
        match fixt!(Header) {
            Header::Create(_) => Header::Create(fixt!(Create, PublicCurve)),
            Header::Update(_) => Header::Update(fixt!(Update, PublicCurve)),
            other_type => other_type,
        }
    };
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
                let ec = CreateFixturator::new_indexed(get_fixt_curve!().clone(), get_fixt_index!()).next().unwrap();
                NewEntryHeader::Create(ec)
            },
            NewEntryHeader::Update(_) => {
                let eu = UpdateFixturator::new_indexed(get_fixt_curve!().clone(), get_fixt_index!()).next().unwrap();
                NewEntryHeader::Update(eu)
            },
        }
    };
);

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
