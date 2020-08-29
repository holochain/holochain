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
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holochain_zome_types::header::builder::HeaderBuilderCommon;
use holochain_zome_types::header::AgentValidationPkg;
use holochain_zome_types::header::AppEntryType;
use holochain_zome_types::header::ChainClose;
use holochain_zome_types::header::ChainOpen;
use holochain_zome_types::header::Dna;
use holochain_zome_types::header::ElementDelete;
use holochain_zome_types::header::EntryCreate;
use holochain_zome_types::header::EntryType;
use holochain_zome_types::header::EntryUpdate;
use holochain_zome_types::header::Header;
use holochain_zome_types::header::InitZomesComplete;
use holochain_zome_types::header::LinkAdd;
use holochain_zome_types::header::LinkRemove;
use holochain_zome_types::header::ZomeId;
use holochain_zome_types::timestamp::Timestamp as ZomeTimestamp;

use holo_hash::fixt::EntryHashFixturator;
use holo_hash::fixt::HeaderHashFixturator;
use holo_hash::fixt::WasmHashFixturator;

use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holochain_keystore::Signature;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::capability::CapAccess;
use holochain_zome_types::capability::CapClaim;
use holochain_zome_types::capability::CapGrant;
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::capability::CurryPayloads;
use holochain_zome_types::capability::GrantedFunction;
use holochain_zome_types::capability::GrantedFunctions;
use holochain_zome_types::capability::ZomeCallCapGrant;
use holochain_zome_types::capability::CAP_SECRET_BYTES;
use holochain_zome_types::crdt::CrdtType;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::entry_def::EntryDefId;
use holochain_zome_types::entry_def::EntryDefs;
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::entry_def::RequiredValidations;
use holochain_zome_types::link::LinkTag;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::Entry;
use rand::seq::IteratorRandom;
use rand::thread_rng;
use rand::Rng;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::iter::Iterator;

/// a curve to spit out Entry::App values
pub struct AppEntry;

/// A curve to make headers have public entry types
pub struct PublicCurve;

fixturator!(
    Zome;
    constructor fn from_hash(WasmHash);
);

newtype_fixturator!(ZomeName<String>);

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
    curve Predictable [self.0.index as u8; CAP_SECRET_BYTES].into();
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
    EntryVisibility;
    unit variants [ Public Private ] empty Public;
);

fixturator!(
    AppEntryType;
    constructor fn new(U8, U8, EntryVisibility);
);

impl Iterator for AppEntryTypeFixturator<EntryVisibility> {
    type Item = AppEntryType;
    fn next(&mut self) -> Option<Self::Item> {
        let app_entry = AppEntryTypeFixturator::new(Unpredictable).next().unwrap();
        Some(AppEntryType::new(
            app_entry.id(),
            app_entry.zome_id(),
            self.0.curve,
        ))
    }
}

fixturator!(
    Timestamp;
    constructor fn now();
);

fn from_timestamp(ts: Timestamp) -> ZomeTimestamp {
    ts.into()
}

fixturator!(
    ZomeTimestamp;
    vanilla fn from_timestamp(Timestamp);
);

fixturator!(
    HeaderBuilderCommon;
    constructor fn new(AgentPubKey, ZomeTimestamp, u32, HeaderHash);
);

newtype_fixturator!(Signature<Bytes>);

fixturator!(
    MigrateAgent;
    unit variants [ Open Close ] empty Close;
);

fixturator!(
    GrantedFunction;
    curve Empty (
        ZomeNameFixturator::new_indexed(Empty, self.0.index).next().unwrap(),
        StringFixturator::new_indexed(Empty, self.0.index).next().unwrap()
    );
    curve Unpredictable (
        ZomeNameFixturator::new_indexed(Unpredictable, self.0.index).next().unwrap(),
        StringFixturator::new_indexed(Unpredictable, self.0.index).next().unwrap()
    );
    curve Predictable (
        ZomeNameFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
        StringFixturator::new_indexed(Predictable, self.0.index).next().unwrap()
    );
);

fixturator!(
    CurryPayloads;
    curve Empty CurryPayloads(BTreeMap::new());
    curve Unpredictable {
        let mut rng = rand::thread_rng();
        let number_of_payloads = rng.gen_range(0, 5);

        let mut payloads: BTreeMap<GrantedFunction, SerializedBytes> = BTreeMap::new();
        let mut granted_function_fixturator = GrantedFunctionFixturator::new_indexed(Unpredictable, self.0.index);
        let mut sb_fixturator = SerializedBytesFixturator::new_indexed(Unpredictable, self.0.index);
        for _ in 0..number_of_payloads {
            payloads.insert(granted_function_fixturator.next().unwrap(), sb_fixturator.next().unwrap());
        }
        CurryPayloads(payloads)
    };
    curve Predictable {
        let mut rng = rand::thread_rng();
        let number_of_payloads = rng.gen_range(0, 5);

        let mut payloads: BTreeMap<GrantedFunction, SerializedBytes> = BTreeMap::new();
        let mut granted_function_fixturator = GrantedFunctionFixturator::new_indexed(Predictable, self.0.index);
        let mut sb_fixturator = SerializedBytesFixturator::new_indexed(Predictable, self.0.index);
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
            },
            CurryPayloadsFixturator::new(Empty).next().unwrap(),
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
            CurryPayloadsFixturator::new(Unpredictable).next().unwrap(),
        )
    },
    {
        ZomeCallCapGrant::new(
            StringFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            CapAccessFixturator::new_indexed(Predictable, self.0.index)
                .next()
                .unwrap(),
            {
                let mut granted_functions: GrantedFunctions = HashSet::new();
                for _ in 0..self.0.index % 3 {
                    granted_functions
                        .insert(GrantedFunctionFixturator::new(Predictable).next().unwrap());
                }
                granted_functions
            },
            CurryPayloadsFixturator::new(Predictable).next().unwrap(),
        )
    }
);

fixturator!(
    CapAccess;

    enum [ Unrestricted Transferable Assigned ];

    curve Empty {
        match CapAccessVariant::random() {
            CapAccessVariant::Unrestricted => CapAccess::from(()),
            CapAccessVariant::Transferable => CapAccess::from(CapSecretFixturator::new_indexed(Empty, self.0.index).next().unwrap()),
            CapAccessVariant::Assigned => CapAccess::from((
                CapSecretFixturator::new_indexed(Empty, self.0.index).next().unwrap(),
                HashSet::new()
            ))
        }
    };

    curve Unpredictable {
        match CapAccessVariant::random() {
            CapAccessVariant::Unrestricted => CapAccess::from(()),
            CapAccessVariant::Transferable => {
                CapAccess::from(CapSecretFixturator::new_indexed(Unpredictable, self.0.index).next().unwrap())
            },
            CapAccessVariant::Assigned => {
                let mut rng = rand::thread_rng();
                let number_of_assigned = rng.gen_range(0, 5);

                CapAccess::from((
                    CapSecretFixturator::new_indexed(Unpredictable, self.0.index).next().unwrap(),
                    {
                        let mut set: HashSet<AgentPubKey> = HashSet::new();
                        for _ in 0..number_of_assigned {
                            set.insert(AgentPubKeyFixturator::new_indexed(Unpredictable, self.0.index).next().unwrap());
                        }
                        set
                    }
                ))
            }
        }
    };

    curve Predictable {
        match CapAccessVariant::nth(self.0.index) {
            CapAccessVariant::Unrestricted => CapAccess::from(()),
            CapAccessVariant::Transferable => CapAccess::from(CapSecretFixturator::new_indexed(Predictable, self.0.index).next().unwrap()),
            CapAccessVariant::Assigned => CapAccess::from((
                CapSecretFixturator::new_indexed(Predictable, self.0.index).next().unwrap(),
            {
                let mut set: HashSet<AgentPubKey> = HashSet::new();
                for _ in 0..self.0.index % 3 {
                    set.insert(AgentPubKeyFixturator::new_indexed(Predictable, self.0.index).next().unwrap());
                }
                set
            }))
        }
    };
);

fixturator!(
    CapGrant;
    variants [ Authorship(AgentPubKey) ZomeCall(ZomeCallCapGrant) ];
);

fixturator!(
    Entry;
    variants [
        Agent(AgentPubKey)
        App(SerializedBytes)
        CapClaim(CapClaim)
        CapGrant(ZomeCallCapGrant)
    ];

    curve AppEntry {
        Entry::App(SerializedBytesFixturator::new_indexed(Unpredictable, self.0.index).next().unwrap())
    };
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
    constructor fn new(EntryDefId, EntryVisibility, CrdtType, RequiredValidations);
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
        name: StringFixturator::new_indexed(Empty, self.0.index)
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Empty, self.0.index)
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Empty, self.0.index)
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Empty, self.0.index)
            .next()
            .unwrap(),
    };

    curve Unpredictable DnaDef {
        name: StringFixturator::new_indexed(Unpredictable, self.0.index)
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Unpredictable, self.0.index)
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Unpredictable, self.0.index)
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Unpredictable, self.0.index)
            .next()
            .unwrap(),
    };

    curve Predictable DnaDef {
        name: StringFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap(),
        uuid: StringFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap(),
        zomes: ZomesFixturator::new_indexed(Predictable, self.0.index)
            .next()
            .unwrap(),
    };
);

fixturator!(
    LinkTag; from Bytes;
);

fixturator!(
    Dna;
    constructor fn from_builder(DnaHash, HeaderBuilderCommon);
);

fixturator!(
    LinkRemove;
    constructor fn from_builder(HeaderBuilderCommon, HeaderHash, EntryHash);
);

fixturator!(
    LinkAdd;
    constructor fn from_builder(HeaderBuilderCommon, EntryHash, EntryHash, u8, LinkTag);
);

pub struct KnownLinkAdd {
    pub base_address: EntryHash,
    pub target_address: EntryHash,
    pub tag: LinkTag,
    pub zome_id: ZomeId,
}

pub struct KnownLinkRemove {
    pub link_add_address: holo_hash::HeaderHash,
}

impl Iterator for LinkAddFixturator<KnownLinkAdd> {
    type Item = LinkAdd;
    fn next(&mut self) -> Option<Self::Item> {
        let mut f = fixt!(LinkAdd);
        f.base_address = self.0.curve.base_address.clone();
        f.target_address = self.0.curve.target_address.clone();
        f.tag = self.0.curve.tag.clone();
        f.zome_id = self.0.curve.zome_id;
        Some(f)
    }
}

impl Iterator for LinkRemoveFixturator<KnownLinkRemove> {
    type Item = LinkRemove;
    fn next(&mut self) -> Option<Self::Item> {
        let mut f = fixt!(LinkRemove);
        f.link_add_address = self.0.curve.link_add_address.clone();
        Some(f)
    }
}

pub type MaybeSerializedBytes = Option<SerializedBytes>;

fixturator! {
    MaybeSerializedBytes;
    enum [ Some None ];
    curve Empty MaybeSerializedBytes::None;
    curve Unpredictable match MaybeSerializedBytesVariant::random() {
        MaybeSerializedBytesVariant::None => MaybeSerializedBytes::None,
        MaybeSerializedBytesVariant::Some => MaybeSerializedBytes::Some(fixt!(SerializedBytes)),
    };
    curve Predictable match MaybeSerializedBytesVariant::nth(self.0.index) {
        MaybeSerializedBytesVariant::None => MaybeSerializedBytes::None,
        MaybeSerializedBytesVariant::Some => MaybeSerializedBytes::Some(SerializedBytesFixturator::new_indexed(Predictable, self.0.index).next().unwrap()),
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
    curve Predictable match EntryTypeVariant::nth(self.0.index) {
        EntryTypeVariant::AgentPubKey => EntryType::AgentPubKey,
        EntryTypeVariant::App => EntryType::App(AppEntryTypeFixturator::new_indexed(Predictable, self.0.index).next().unwrap()),
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
    ChainOpen;
    constructor fn from_builder(HeaderBuilderCommon, DnaHash);
);

fixturator!(
    ChainClose;
    constructor fn from_builder(HeaderBuilderCommon, DnaHash);
);

fixturator!(
    EntryCreate;
    constructor fn from_builder(HeaderBuilderCommon, EntryType, EntryHash);

    curve PublicCurve {
        let mut ec = fixt!(EntryCreate);
        ec.entry_type = fixt!(EntryType, PublicCurve);
        ec
    };
);

fixturator!(
    EntryUpdate;
    constructor fn from_builder(HeaderBuilderCommon, EntryHash, HeaderHash, EntryType, EntryHash);

    curve PublicCurve {
        let mut eu = fixt!(EntryUpdate);
        eu.entry_type = fixt!(EntryType, PublicCurve);
        eu
    };
);

fixturator!(
    ElementDelete;
    constructor fn from_builder(HeaderBuilderCommon, HeaderHash, EntryHash);
);

fixturator!(
    Header;
    variants [
        Dna(Dna)
        AgentValidationPkg(AgentValidationPkg)
        InitZomesComplete(InitZomesComplete)
        LinkAdd(LinkAdd)
        LinkRemove(LinkRemove)
        ChainOpen(ChainOpen)
        ChainClose(ChainClose)
        EntryCreate(EntryCreate)
        EntryUpdate(EntryUpdate)
        ElementDelete(ElementDelete)
    ];

    curve PublicCurve {
        match fixt!(Header) {
            Header::EntryCreate(_) => Header::EntryCreate(fixt!(EntryCreate, PublicCurve)),
            Header::EntryUpdate(_) => Header::EntryUpdate(fixt!(EntryUpdate, PublicCurve)),
            other_type => other_type,
        }
    };
);

fixturator!(
    NewEntryHeader;
    variants [
        Create(EntryCreate)
        Update(EntryUpdate)
    ];


    curve PublicCurve {
        match fixt!(NewEntryHeader) {
            NewEntryHeader::Create(_) => NewEntryHeader::Create(fixt!(EntryCreate, PublicCurve)),
            NewEntryHeader::Update(_) => NewEntryHeader::Update(fixt!(EntryUpdate, PublicCurve)),
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
