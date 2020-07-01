//! Fixture definitions for holochain_types structs

// FIXME (aka fixtme, haha, get it?) move other fixturators from this crate into this module

#![allow(missing_docs)]

use crate::composite_hash::AnyDhtHash;
use crate::composite_hash::EntryHash;
use crate::dna::zome::Zome;
use crate::dna::DnaDef;
use crate::dna::Zomes;
use crate::header::AgentValidationPkg;
use crate::header::ChainClose;
use crate::header::ChainOpen;
use crate::header::EntryCreate;
use crate::header::EntryDelete;
use crate::header::EntryType;
use crate::header::EntryUpdate;
use crate::header::InitZomesComplete;
use crate::header::LinkAdd;
use crate::header::{builder::HeaderBuilderCommon, AppEntryType, IntendedFor};
use crate::header::{Dna, LinkRemove, ZomeId};
use crate::link::Tag;
use crate::Timestamp;
use fixt::prelude::*;
use holo_hash::AgentPubKeyFixturator;
use holo_hash::DnaHashFixturator;
use holo_hash::EntryContentHashFixturator;
use holo_hash::HeaderHashFixturator;
use holo_hash::WasmHashFixturator;
use holochain_keystore::Signature;
use holochain_serialized_bytes::SerializedBytes;
use holochain_zome_types::capability::CapAccess;
use holochain_zome_types::capability::CapClaim;
use holochain_zome_types::capability::CapGrant;
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::capability::GrantedFunctions;
use holochain_zome_types::capability::ZomeCallCapGrant;
use holochain_zome_types::crdt::CrdtType;
use holochain_zome_types::entry_def::EntryDef;
use holochain_zome_types::entry_def::EntryDefId;
use holochain_zome_types::entry_def::EntryDefs;
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::entry_def::RequiredValidations;
use holochain_zome_types::migrate_agent::MigrateAgent;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::Entry;
use rand::seq::IteratorRandom;
use rand::thread_rng;
use rand::Rng;
use std::collections::BTreeMap;
use std::collections::HashSet;

/// a curve to spit out Entry::App values
pub struct AppEntry;

fixturator!(
    Zome;
    constructor fn from_hash(WasmHash);
);

newtype_fixturator!(ZomeName<String>);

fixturator!(
    CapSecret;
    from String;
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

fixturator!(
    HeaderBuilderCommon;
    constructor fn new(AgentPubKey, Timestamp, u32, HeaderHash);
);

newtype_fixturator!(Signature<Bytes>);

fixturator!(
    IntendedFor;
    unit variants [ Entry Header ] empty Entry;
);

fixturator!(
    EntryHash;
    variants [
        Entry(EntryContentHash)
        Agent(AgentPubKey)
    ];
);

fixturator!(
    MigrateAgent;
    unit variants [ Open Close ] empty Close;
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

                let mut granted_functions: GrantedFunctions = BTreeMap::new();
                for _ in 0..number_of_zomes {
                    let number_of_functions = rng.gen_range(0, 5);
                    let mut zome_functions = vec![];
                    for _ in 0..number_of_functions {
                        zome_functions.push(StringFixturator::new(Empty).next().unwrap());
                    }
                    granted_functions.insert(
                        ZomeNameFixturator::new(Empty).next().unwrap(),
                        zome_functions,
                    );
                }
                granted_functions
            },
        )
    },
    {
        ZomeCallCapGrant::new(
            StringFixturator::new(Unpredictable).next().unwrap(),
            CapAccessFixturator::new(Unpredictable).next().unwrap(),
            {
                let mut rng = rand::thread_rng();
                let number_of_zomes = rng.gen_range(0, 5);

                let mut granted_functions: GrantedFunctions = BTreeMap::new();
                for _ in 0..number_of_zomes {
                    let number_of_functions = rng.gen_range(0, 5);
                    let mut zome_functions = vec![];
                    for _ in 0..number_of_functions {
                        zome_functions.push(StringFixturator::new(Unpredictable).next().unwrap());
                    }
                    granted_functions.insert(
                        ZomeNameFixturator::new(Unpredictable).next().unwrap(),
                        zome_functions,
                    );
                }
                granted_functions
            },
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
                let mut granted_functions: GrantedFunctions = BTreeMap::new();
                for _ in 0..self.0.index % 3 {
                    let number_of_functions = self.0.index % 3;
                    let mut zome_functions = vec![];
                    for _ in 0..number_of_functions {
                        zome_functions.push(StringFixturator::new(Predictable).next().unwrap());
                    }
                    granted_functions.insert(
                        ZomeNameFixturator::new(Predictable).next().unwrap(),
                        zome_functions,
                    );
                }
                granted_functions
            },
        )
    }
);

fixturator!(
    CapAccess;

    enum [ Unrestricted Transferable Assigned ];

    curve Empty {
        match CapAccessVariant::random() {
            Unrestricted => CapAccess::unrestricted(),
            Transferable => CapAccess::transferable(),
            Assigned => CapAccess::assigned({
                let mut set = HashSet::new();
                set.insert(fixt!(AgentPubKey, Empty).into());
                set
            })
        }
    };

    curve Unpredictable {
        match CapAccessVariant::random() {
            Unrestricted => CapAccess::unrestricted(),
            Transferable => CapAccess::transferable(),
            Assigned => CapAccess::assigned({
                let mut set = HashSet::new();
                set.insert(fixt!(AgentPubKey).into());
                set
            })
        }
    };

    curve Predictable {
        match CapAccessVariant::nth(self.0.index) {
            Unrestricted => CapAccess::unrestricted(),
            Transferable => CapAccess::transferable(),
            Assigned => CapAccess::assigned({
                let mut set = HashSet::new();
                set.insert(AgentPubKeyFixturator::new_indexed(Predictable, self.0.index).next().unwrap().into());
                set
            })
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
    Tag; from Bytes;
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
    constructor fn from_builder(HeaderBuilderCommon, EntryHash, EntryHash, u8, Tag);
);

pub struct KnownLinkAdd {
    pub base_address: EntryHash,
    pub target_address: EntryHash,
    pub tag: Tag,
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
);

fixturator!(
    AnyDhtHash;
    variants [
        EntryContent(EntryContentHash)
        Agent(AgentPubKey)
        Header(HeaderHash)
    ];
);

fixturator!(
    EntryUpdate;
    constructor fn from_builder(HeaderBuilderCommon, IntendedFor, HeaderHash, EntryType, EntryHash);
);

fixturator!(
    EntryDelete;
    constructor fn from_builder(HeaderBuilderCommon, HeaderHash);
);
