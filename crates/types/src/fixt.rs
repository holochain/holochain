//! Fixture definitions for holochain_types structs

// FIXME (aka fixtme, haha, get it?) move other fixturators from this crate into this module

use crate::dna::zome::Zome;
use crate::header::{builder::HeaderBuilderCommon, AppEntryType};
use crate::Timestamp;
use fixt::prelude::*;
use holo_hash::AgentPubKeyFixturator;
use holo_hash::HeaderHashFixturator;
use holo_hash::WasmHashFixturator;
use holochain_zome_types::capability::CapClaim;
use holochain_zome_types::capability::CapSecret;
use holochain_zome_types::entry_def::EntryVisibility;

fixturator!(
    Zome;
    constructor fn from_hash(WasmHash);
);

fixturator!(
    CapSecret;
    from String;
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
    constructor fn new(Bytes, U8, EntryVisibility);
);

fixturator!(
    Timestamp;
    constructor fn now();
);

fixturator!(
    HeaderBuilderCommon;
    constructor fn new(AgentPubKey, Timestamp, u32, HeaderHash);
);
