//! Fixturators for zome types

use crate::prelude::*;
use ::fixt::prelude::*;
use ::fixt::*;
use holo_hash::fixt::*;
use holo_hash::EntryHash;
use holochain_serialized_bytes::prelude::SerializedBytes;
use rand::Rng;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::Arc;
use std::time::Duration;

fixturator!(
    ExternIO;
    from Bytes;
);

fixturator!(
    CellId;
    constructor fn new(DnaHash, AgentPubKey);
);

// Create random timestamps that are guaranteed to be valid UTC date and time (see datetime
// from_timestamp implementation)
//  - valid +'ve seconds, convertible to +'ve i32 when divided by days
//  - valid nanoseconds
fixturator!(
    Timestamp;
    curve Empty {
        Timestamp::from_micros((I64Fixturator::new(Empty).next().unwrap().abs()
           % ((i32::MAX as i64) * 86_400)).abs())
    };
    curve Unpredictable {
        Timestamp::from_micros((I64Fixturator::new(Unpredictable).next().unwrap()
           % ((i32::MAX as i64) * 86_400)).abs())
    };
    curve Predictable {
        Timestamp::from_micros((I64Fixturator::new(Predictable).next().unwrap()
           % ((i32::MAX as i64) * 86_400)).abs())
    };
);

fixturator!(
    EntryVisibility;
    unit variants [ Public Private ] empty Public;
);

fixturator!(
    AppEntryDef;
    constructor fn new(U8, U8, EntryVisibility);
);

impl Iterator for AppEntryDefFixturator<EntryVisibility> {
    type Item = AppEntryDef;
    fn next(&mut self) -> Option<Self::Item> {
        let app_entry = AppEntryDefFixturator::new(Unpredictable).next().unwrap();
        Some(AppEntryDef::new(
            app_entry.entry_index(),
            app_entry.zome_index(),
            self.0.curve,
        ))
    }
}

/// Alias
pub type MaybeMembraneProof = Option<Arc<SerializedBytes>>;

fixturator!(
    ActionBuilderCommon;
    constructor fn new(AgentPubKey, Timestamp, u32, ActionHash);
);

fixturator!(
    DeleteLink;
    constructor fn from_builder(ActionBuilderCommon, ActionHash, AnyLinkableHash);
);

fixturator!(
    CreateLink;
    constructor fn from_builder(ActionBuilderCommon, AnyLinkableHash, AnyLinkableHash, ZomeIndex, LinkType, LinkTag);
);

fixturator!(
    LinkType; constructor fn new(u8);
);

fixturator!(
    LinkTag; from Bytes;
);

pub struct KnownCreateLink {
    pub author: AgentPubKey,
    pub base_address: AnyLinkableHash,
    pub target_address: AnyLinkableHash,
    pub tag: LinkTag,
    pub zome_index: ZomeIndex,
    pub link_type: LinkType,
}

pub struct KnownDeleteLink {
    pub link_add_address: holo_hash::ActionHash,
    pub base_address: AnyLinkableHash,
}

impl Iterator for CreateLinkFixturator<KnownCreateLink> {
    type Item = CreateLink;
    fn next(&mut self) -> Option<Self::Item> {
        let mut f = fixt!(CreateLink);
        f.author = self.0.curve.author.clone();
        f.base_address = self.0.curve.base_address.clone();
        f.target_address = self.0.curve.target_address.clone();
        f.tag = self.0.curve.tag.clone();
        f.zome_index = self.0.curve.zome_index;
        f.link_type = self.0.curve.link_type;
        Some(f)
    }
}

impl Iterator for DeleteLinkFixturator<KnownDeleteLink> {
    type Item = DeleteLink;
    fn next(&mut self) -> Option<Self::Item> {
        let mut f = fixt!(DeleteLink);
        f.link_add_address = self.0.curve.link_add_address.clone();
        f.base_address = self.0.curve.base_address.clone();
        Some(f)
    }
}

/// a curve to spit out Entry::App values
#[derive(Clone)]
pub struct AppEntry;

/// A curve to make actions have public entry types
#[derive(Clone)]
pub struct PublicCurve;

fixturator!(
    ZomeName;
    from String;
);

fixturator!(
    with_vec 0 5;
    FunctionName;
    from String;
);

fixturator!(
    CapSecret;
    curve Empty [0; CAP_SECRET_BYTES].into();
    curve Unpredictable {
        let mut rng = rng();
        let upper = rng.random::<[u8; CAP_SECRET_BYTES / 2]>();
        let lower = rng.random::<[u8; CAP_SECRET_BYTES / 2]>();
        let mut inner = [0; CAP_SECRET_BYTES];
        inner[..CAP_SECRET_BYTES / 2].copy_from_slice(&lower);
        inner[CAP_SECRET_BYTES / 2..].copy_from_slice(&upper);
        inner.into()
    };
    curve Predictable [get_fixt_index!() as u8; CAP_SECRET_BYTES].into();
);

fixturator!(
    ZomeIndex;
    from u8;
);

fixturator!(
    ScopedZomeTypesSet;
    constructor fn default();;
);

fixturator!(
    ZomeInfo;
    constructor fn new(ZomeName, ZomeIndex, SerializedBytes, EntryDefs, FunctionNameVec, ScopedZomeTypesSet);
);

fixturator!(
    AgentInfo;
    curve Empty AgentInfo {
        agent_initial_pubkey: fixt!(AgentPubKey, Empty),
        chain_head: (fixt!(ActionHash, Empty), fixt!(u32, Empty), fixt!(Timestamp, Empty)),
    };
    curve Unpredictable AgentInfo {
        agent_initial_pubkey: fixt!(AgentPubKey, Unpredictable),
        chain_head: (fixt!(ActionHash, Unpredictable), fixt!(u32, Unpredictable), fixt!(Timestamp, Unpredictable)),
    };
    curve Predictable AgentInfo {
        agent_initial_pubkey: fixt!(AgentPubKey, Predictable),
        chain_head: (fixt!(ActionHash, Predictable), fixt!(u32, Predictable), fixt!(Timestamp, Predictable)),
    };
);

fixturator!(
    CapClaim;
    constructor fn new(String, AgentPubKey, CapSecret);
);

newtype_fixturator!(Signature<SixtyFourBytes>);

pub type SignatureVec = Vec<Signature>;
fixturator!(
    SignatureVec;
    curve Empty vec![];
    curve Unpredictable {
        let min_len = 0;
        let max_len = 5;
        let mut rng = rng();
        let len = rng.random_range(min_len..max_len);
        let mut signature_fixturator = SignatureFixturator::new(Unpredictable);
        let mut signatures = vec![];
        for _ in 0..len {
            signatures.push(signature_fixturator.next().unwrap());
        }
        signatures
    };
    curve Predictable {
        let mut index = get_fixt_index!();
        let mut signature_fixturator = SignatureFixturator::new_indexed(Predictable, index);
        let mut signatures = vec![];
        for _ in 0..3 {
            signatures.push(signature_fixturator.next().unwrap());
        }
        index += 1;
        set_fixt_index!(index);
        signatures
    };
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
        let mut rng = rng();
        let number_of_payloads = rng.random_range(0..5);

        let mut payloads: BTreeMap<GrantedFunction, SerializedBytes> = BTreeMap::new();
        let mut granted_function_fixturator = GrantedFunctionFixturator::new_indexed(Unpredictable, get_fixt_index!());
        let mut sb_fixturator = SerializedBytesFixturator::new_indexed(Unpredictable, get_fixt_index!());
        for _ in 0..number_of_payloads {
            payloads.insert(granted_function_fixturator.next().unwrap(), sb_fixturator.next().unwrap());
        }
        CurryPayloads(payloads)
    };
    curve Predictable {
        let mut rng = rand::rng();
        let number_of_payloads = rng.random_range(0..5);

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
    ZomeCallCapGrant;
    curve Empty {
        ZomeCallCapGrant::new(
            StringFixturator::new(Empty).next().unwrap(),
            CapAccessFixturator::new(Empty).next().unwrap(),
            {
                let mut rng = rng();
                let number_of_zomes = rng.random_range(0..5);

                let mut fns = HashSet::new();
                for _ in 0..number_of_zomes {
                    fns.insert(GrantedFunctionFixturator::new(Empty).next().unwrap());
                }
                GrantedFunctions::Listed(fns)
            }, // CurryPayloadsFixturator::new(Empty).next().unwrap(),
        )
    };
    curve Unpredictable {
        ZomeCallCapGrant::new(
            StringFixturator::new(Unpredictable).next().unwrap(),
            CapAccessFixturator::new(Unpredictable).next().unwrap(),
            {
                let mut rng = rand::rng();
                let number_of_zomes = rng.random_range(0..5);

                let mut fns = HashSet::new();
                for _ in 0..number_of_zomes {
                    fns.insert(
                    GrantedFunctionFixturator::new(Unpredictable)
                        .next()
                        .unwrap(),
                    );
                }
                GrantedFunctions::Listed(fns)
            },
            // CurryPayloadsFixturator::new(Unpredictable).next().unwrap(),
        )
    };
    curve Predictable {
        ZomeCallCapGrant::new(
            StringFixturator::new_indexed(Predictable, get_fixt_index!())
                .next()
                .unwrap(),
            CapAccessFixturator::new_indexed(Predictable, get_fixt_index!())
                .next()
                .unwrap(),
            {
                if get_fixt_index!() %2 == 0{
                    let mut fns = HashSet::new();
                    for _ in 0..get_fixt_index!() % 3 {
                        fns.insert(GrantedFunctionFixturator::new(Predictable).next().unwrap());
                    }
                    GrantedFunctions::Listed(fns)
                } else {
                    GrantedFunctions::All
                }
            }
        )
    };
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
                BTreeSet::new()
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
                let mut rng = rand::rng();
                let number_of_assigned = rng.random_range(0..5);

                CapAccess::from((
                    CapSecretFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap(),
                    {
                        let mut set: BTreeSet<AgentPubKey> = BTreeSet::new();
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
                let mut set: BTreeSet<AgentPubKey> = BTreeSet::new();
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

pub fn record_with_no_entry(signature: Signature, action: Action) -> Record {
    let shh = SignedActionHashed::with_presigned(
        holo_hash::HoloHashed::from_content_sync(action),
        signature,
    );
    Record::new(shh, RecordEntry::NA)
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
    constructor fn new(EntryDefId, EntryVisibility, RequiredValidations, bool);
);

fixturator!(
    EntryDefs;
    curve Empty Vec::new().into();
    curve Unpredictable {
        let mut rng = rand::rng();
        let number_of_defs = rng.random_range(0..5);

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
    Dna;
    constructor fn from_builder(DnaHash, ActionBuilderCommon);
);

fixturator! {
    MaybeMembraneProof;
    enum [ Some None ];
    curve Empty MaybeMembraneProof::None;
    curve Unpredictable match MaybeMembraneProofVariant::random() {
        MaybeMembraneProofVariant::None => MaybeMembraneProof::None,
        MaybeMembraneProofVariant::Some => MaybeMembraneProof::Some(Arc::new(fixt!(SerializedBytes))),
    };
    curve Predictable match MaybeMembraneProofVariant::nth(get_fixt_index!()) {
        MaybeMembraneProofVariant::None => MaybeMembraneProof::None,
        MaybeMembraneProofVariant::Some => MaybeMembraneProof::Some(Arc::new(SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap())),
    };
}

fixturator! {
    EntryType;
    enum [ AgentPubKey App CapClaim CapGrant ];
    curve Empty EntryType::AgentPubKey;
    curve Unpredictable match EntryTypeVariant::random() {
        EntryTypeVariant::AgentPubKey => EntryType::AgentPubKey,
        EntryTypeVariant::App => EntryType::App(fixt!(AppEntryDef)),
        EntryTypeVariant::CapClaim => EntryType::CapClaim,
        EntryTypeVariant::CapGrant => EntryType::CapGrant,
    };
    curve Predictable match EntryTypeVariant::nth(get_fixt_index!()) {
        EntryTypeVariant::AgentPubKey => EntryType::AgentPubKey,
        EntryTypeVariant::App => EntryType::App(AppEntryDefFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()),
        EntryTypeVariant::CapClaim => EntryType::CapClaim,
        EntryTypeVariant::CapGrant => EntryType::CapGrant,
    };
    curve PublicCurve {
        let app_entry_def = fixt!(AppEntryDef);
        EntryType::App(AppEntryDef::new(app_entry_def.entry_index(), app_entry_def.zome_index(), EntryVisibility::Public))
    };
}

fixturator!(
    AgentValidationPkg;
    constructor fn from_builder(ActionBuilderCommon, MaybeMembraneProof);
);

fixturator!(
    InitZomesComplete;
    constructor fn from_builder(ActionBuilderCommon);
);

fixturator!(
    MigrationTarget;
    variants [ Dna(DnaHash) Agent(AgentPubKey) ];
);

fixturator!(
    OpenChain;
    constructor fn from_builder(ActionBuilderCommon, MigrationTarget, ActionHash);
);

fixturator!(
    CloseChain;
    constructor fn from_builder(ActionBuilderCommon, MigrationTarget);
);

fixturator!(
    Create;
    constructor fn from_builder(ActionBuilderCommon, EntryType, EntryHash);

    curve PublicCurve {
        let mut ec = fixt!(Create);
        ec.entry_type = fixt!(EntryType, PublicCurve);
        ec
    };
    curve EntryType {
        let mut ec = CreateFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        ec.entry_type = get_fixt_curve!();
        ec
    };
    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) | Entry::CounterSign(_, _) => EntryType::App(AppEntryDefFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
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
    constructor fn from_builder(ActionBuilderCommon, EntryHash, ActionHash, EntryType, EntryHash);

    curve PublicCurve {
        let mut eu = fixt!(Update);
        eu.entry_type = fixt!(EntryType, PublicCurve);
        eu
    };

    curve EntryType {
        let mut eu = UpdateFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        eu.entry_type = get_fixt_curve!();
        eu
    };

    curve EntryTypeEntryHash {
        let mut u = UpdateFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap();
        u.entry_type = get_fixt_curve!().0;
        u.entry_hash = get_fixt_curve!().1;
        u
    };

    curve Entry {
        let et = match get_fixt_curve!() {
            Entry::App(_) | Entry::CounterSign(_, _) => EntryType::App(AppEntryDefFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()),
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
    constructor fn from_builder(ActionBuilderCommon, ActionHash, EntryHash);
);

use crate::dht_v2::{
    ActionData, ActionHeader, AgentValidationPkgData, CloseChainData, CreateData, CreateLinkData,
    DeleteData, DeleteLinkData, DnaData, InitZomesCompleteData, OpenChainData, UpdateData,
};

// Build an [`Action`] directly from a common [`ActionHeader`] plus a
// per-variant [`ActionData`]. The variant is chosen by the fixturator index so
// a sequence exercises all ten variants; the genesis `Dna` action is always the
// first on a chain, so it carries `action_seq == 0` and no `prev_action`.
macro_rules! action_for_curve {
    ($curve:expr, $index:expr, $variant:expr) => {{
        let index = $index;
        let author = AgentPubKeyFixturator::new_indexed($curve, index)
            .next()
            .unwrap();
        let timestamp = TimestampFixturator::new_indexed($curve, index)
            .next()
            .unwrap();
        let action_seq = U32Fixturator::new_indexed($curve, index).next().unwrap();
        let prev_action = ActionHashFixturator::new_indexed($curve, index)
            .next()
            .unwrap();
        let variant = $variant;
        let header = ActionHeader {
            author,
            timestamp,
            action_seq: if variant == 0 { 0 } else { action_seq },
            prev_action: if variant == 0 {
                None
            } else {
                Some(prev_action)
            },
        };
        let data = match variant {
            0 => ActionData::Dna(DnaData {
                dna_hash: DnaHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
            }),
            1 => ActionData::AgentValidationPkg(AgentValidationPkgData {
                membrane_proof: if index % 2 == 0 {
                    None
                } else {
                    Some(std::sync::Arc::new(
                        SerializedBytesFixturator::new_indexed($curve, index)
                            .next()
                            .unwrap(),
                    ))
                },
            }),
            2 => ActionData::InitZomesComplete(InitZomesCompleteData {}),
            3 => ActionData::CreateLink(CreateLinkData {
                base_address: AnyLinkableHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                target_address: AnyLinkableHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                zome_index: ZomeIndexFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                link_type: LinkTypeFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                tag: LinkTagFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
            }),
            4 => ActionData::DeleteLink(DeleteLinkData {
                base_address: AnyLinkableHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                link_add_address: ActionHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
            }),
            5 => ActionData::OpenChain(OpenChainData {
                prev_target: MigrationTargetFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                close_hash: ActionHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
            }),
            6 => ActionData::CloseChain(CloseChainData {
                new_target: Some(
                    MigrationTargetFixturator::new_indexed($curve, index)
                        .next()
                        .unwrap(),
                ),
            }),
            7 => ActionData::Create(CreateData {
                entry_type: EntryTypeFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                entry_hash: EntryHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
            }),
            8 => ActionData::Update(UpdateData {
                original_action_address: ActionHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                original_entry_address: EntryHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                entry_type: EntryTypeFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                entry_hash: EntryHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
            }),
            _ => ActionData::Delete(DeleteData {
                deletes_address: ActionHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
                deletes_entry_address: EntryHashFixturator::new_indexed($curve, index)
                    .next()
                    .unwrap(),
            }),
        };
        Action { header, data }
    }};
}

/// Marker curves that make `fixt!(Action, <Variant>Action)` build one specific
/// action variant instead of cycling through all ten.
macro_rules! action_variant_curve {
    ($name:ident) => {
        #[derive(Clone)]
        pub struct $name;
    };
}
action_variant_curve!(DnaAction);
action_variant_curve!(AgentValidationPkgAction);
action_variant_curve!(InitZomesCompleteAction);
action_variant_curve!(CreateLinkAction);
action_variant_curve!(DeleteLinkAction);
action_variant_curve!(OpenChainAction);
action_variant_curve!(CloseChainAction);
action_variant_curve!(CreateAction);
action_variant_curve!(UpdateAction);
action_variant_curve!(DeleteAction);

fixturator!(
    Action;
    curve Empty {
        let index = get_fixt_index!();
        action_for_curve!(Empty, index, index % 10)
    };
    curve Unpredictable {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, index % 10)
    };
    curve Predictable {
        let index = get_fixt_index!();
        action_for_curve!(Predictable, index, index % 10)
    };
    curve DnaAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 0)
    };
    curve AgentValidationPkgAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 1)
    };
    curve InitZomesCompleteAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 2)
    };
    curve CreateLinkAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 3)
    };
    curve DeleteLinkAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 4)
    };
    curve OpenChainAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 5)
    };
    curve CloseChainAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 6)
    };
    curve CreateAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 7)
    };
    curve UpdateAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 8)
    };
    curve DeleteAction {
        let index = get_fixt_index!();
        action_for_curve!(Unpredictable, index, 9)
    };
    curve PublicCurve {
        let index = get_fixt_index!();
        let mut action = action_for_curve!(Unpredictable, index, index % 10);
        // Force entry-creating variants to a public entry type.
        match &mut action.data {
            ActionData::Create(d) => {
                d.entry_type = EntryTypeFixturator::new_indexed(PublicCurve, index)
                    .next()
                    .unwrap();
            }
            ActionData::Update(d) => {
                d.entry_type = EntryTypeFixturator::new_indexed(PublicCurve, index)
                    .next()
                    .unwrap();
            }
            _ => {}
        }
        action
    };
);

// Build a `SignedActionHashed` from an action and signature, hashing the
// action content.
fn signed_action_hashed_from_parts(action: Action, signature: Signature) -> SignedActionHashed {
    SignedActionHashed::with_presigned(holo_hash::HoloHashed::from_content_sync(action), signature)
}

fixturator!(
    with_vec 0 5;
    SignedActionHashed;
    vanilla fn signed_action_hashed_from_parts(Action, Signature);
);

fixturator!(
    Zome;
    constructor fn new(ZomeName, ZomeDef);
);

fixturator!(
    IntegrityZome;
    constructor fn new(ZomeName, IntegrityZomeDef);
);

fixturator!(
    IntegrityZomes;
    curve Empty Vec::new();
    curve Unpredictable {
        // @todo implement unpredictable zomes
        IntegrityZomesFixturator::new(Empty).next().unwrap()
    };
    curve Predictable {
        // @todo implement predictable zomes
        IntegrityZomesFixturator::new(Empty).next().unwrap()
    };
);

fixturator!(
    CoordinatorZome;
    constructor fn new(ZomeName, CoordinatorZomeDef);
);

fixturator!(
    CoordinatorZomes;
    curve Empty Vec::new();
    curve Unpredictable {
        // @todo implement unpredictable zomes
        CoordinatorZomesFixturator::new(Empty).next().unwrap()
    };
    curve Predictable {
        // @todo implement predictable zomes
        CoordinatorZomesFixturator::new(Empty).next().unwrap()
    };
);

fixturator!(
    ZomeDef;
    constructor fn from_hash(WasmHash);
);

fixturator!(
    IntegrityZomeDef;
    constructor fn from_hash(WasmHash);
);

fixturator!(
    CoordinatorZomeDef;
    constructor fn from_hash(WasmHash);
);

fixturator!(
    DnaDef;
    curve Empty DnaDef {
        name: StringFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        modifiers: DnaModifiers {
            network_seed: StringFixturator::new_indexed(Empty, get_fixt_index!())
                .next()
                .unwrap(),
            properties: SerializedBytesFixturator::new_indexed(Empty, get_fixt_index!())
                .next()
                .unwrap(),
        },
        integrity_zomes: IntegrityZomesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        coordinator_zomes: CoordinatorZomesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        #[cfg(feature = "unstable-migration")]
        lineage: Default::default(),
    };

    curve Unpredictable DnaDef {
        name: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        modifiers: DnaModifiers {
            network_seed: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
                .next()
                .unwrap(),
            properties: SerializedBytesFixturator::new_indexed(Unpredictable, get_fixt_index!())
                .next()
                .unwrap(),
        },
        integrity_zomes: IntegrityZomesFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        coordinator_zomes: CoordinatorZomesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        // TODO: non-empty lineage
        #[cfg(feature = "unstable-migration")]
        lineage: Default::default(),
    };

    curve Predictable DnaDef {
        name: StringFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        modifiers: DnaModifiers {
            network_seed: StringFixturator::new_indexed(Predictable, get_fixt_index!())
                .next()
                .unwrap(),
            properties: SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!())
                .next()
                .unwrap(),
        },
        integrity_zomes: IntegrityZomesFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        coordinator_zomes: CoordinatorZomesFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        // TODO: non-empty lineage
        #[cfg(feature = "unstable-migration")]
        lineage: Default::default(),
    };
);

fixturator!(
    Duration;
    curve Empty std::time::Duration::from_nanos(0);
    curve Unpredictable std::time::Duration::from_nanos(
        U64Fixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap()
    );
    curve Predictable std::time::Duration::from_nanos(
        U64Fixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap()
    );
);

fixturator!(
    DnaModifiers;
    curve Empty DnaModifiers {
        network_seed: StringFixturator::new_indexed(Empty, get_fixt_index!()).next().unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Empty, get_fixt_index!())
        .next()
        .unwrap(),
    };

    curve Unpredictable DnaModifiers {
        network_seed: StringFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Unpredictable, get_fixt_index!())
        .next()
        .unwrap(),
    };

    curve Predictable DnaModifiers {
        network_seed: StringFixturator::new_indexed(Predictable, get_fixt_index!()).next().unwrap(),
        properties: SerializedBytesFixturator::new_indexed(Predictable, get_fixt_index!())
        .next()
        .unwrap(),
    };
);

fixturator!(
    DnaInfoV1;
    curve Empty DnaInfoV1 {
        name: StringFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        hash: DnaHashFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        properties: DnaModifiersFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap().properties,
        zome_names: vec![ZomeNameFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap()],
    };

    curve Unpredictable DnaInfoV1 {
        name: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        hash: DnaHashFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        properties: DnaModifiersFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap().properties,
        zome_names: vec![ZomeNameFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap()],
    };

    curve Predictable DnaInfoV1 {
        name: StringFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        hash: DnaHashFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        properties: DnaModifiersFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap().properties,
        zome_names: vec![ZomeNameFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap()],
    };
);

fixturator!(
    DnaInfo;
    curve Empty DnaInfo {
        name: StringFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        hash: DnaHashFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        modifiers: DnaModifiersFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap(),
        zome_names: vec![ZomeNameFixturator::new_indexed(Empty, get_fixt_index!())
            .next()
            .unwrap()],
    };

    curve Unpredictable DnaInfo {
        name: StringFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        hash: DnaHashFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        modifiers: DnaModifiersFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap(),
        zome_names: vec![ZomeNameFixturator::new_indexed(Unpredictable, get_fixt_index!())
            .next()
            .unwrap()],
    };

    curve Predictable DnaInfo {
        name: StringFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        hash: DnaHashFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        modifiers: DnaModifiersFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap(),
        zome_names: vec![ZomeNameFixturator::new_indexed(Predictable, get_fixt_index!())
            .next()
            .unwrap()],
    };
);
