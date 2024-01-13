#![allow(missing_docs)]

use crate::hash_type;
use crate::ActionHash;
use crate::ActionHashB64;
use crate::AgentPubKey;
use crate::AgentPubKeyB64;
use crate::AnyDhtHash;
use crate::AnyDhtHashB64;
use crate::AnyLinkableHash;
use crate::AnyLinkableHashB64;
use crate::DhtOpHash;
use crate::DhtOpHashB64;
use crate::DnaHash;
use crate::DnaHashB64;
use crate::EntryHash;
use crate::EntryHashB64;
use crate::NetIdHash;
use crate::NetIdHashB64;
use crate::WasmHash;
use crate::WasmHashB64;
use ::fixt::prelude::*;
use std::convert::TryFrom;

pub type HashTypeEntry = hash_type::Entry;
pub type HashTypeAnyDht = hash_type::AnyDht;
pub type HashTypeAnyLinkable = hash_type::AnyLinkable;

// TODO: use strum to do this:
//
// fixturator!(
//     HashTypeEntry;
//     unit variants [ Agent Content ] empty Content;
// );

fixturator!(
    HashTypeAnyDht;
    curve Empty HashTypeAnyDht::Action;
    curve Unpredictable HashTypeAnyDht::Action;
    curve Predictable HashTypeAnyDht::Action;
);

fixturator!(
    HashTypeAnyLinkable;
    curve Empty HashTypeAnyLinkable::External;
    curve Unpredictable HashTypeAnyLinkable::Action;
    curve Predictable HashTypeAnyLinkable::Entry;
);

/// A type alias for a `Vec<u8>` whose fixturator is expected to only return
/// a Vec of length 32
pub type ThirtyTwoHashBytes = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8> of 32 bytes
fixturator!(
    ThirtyTwoHashBytes,
    [0; 32].to_vec(),
    {
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        bytes
    },
    {
        let mut index = get_fixt_index!();
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        index += 1;
        set_fixt_index!(index);
        bytes
    }
);

fixturator!(
    with_vec 0 5;
    AgentPubKey;
    curve Empty AgentPubKey::from_raw_32(ThirtyTwoHashBytesFixturator::new_indexed(Empty, get_fixt_index!()).next().unwrap());
    curve Unpredictable AgentPubKey::from_raw_32(ThirtyTwoHashBytesFixturator::new_indexed(Unpredictable, get_fixt_index!()).next().unwrap());
    curve Predictable {
        // these agent keys match what the mock keystore spits out for the first two agents
        // don't mess with this unless you also update the keystore!!!
        let agents = vec![
            AgentPubKey::try_from("uhCAkJCuynkgVdMn_bzZ2ZYaVfygkn0WCuzfFspczxFnZM1QAyXoo")
            .unwrap(),
            AgentPubKey::try_from("uhCAk39SDf7rynCg5bYgzroGaOJKGKrloI1o57Xao6S-U5KNZ0dUH")
                .unwrap(),
        ];
        agents[get_fixt_index!() % agents.len()].clone()
    };
);

fixturator!(
    EntryHash;
    constructor fn from_raw_32(ThirtyTwoHashBytes);
);

fixturator!(
    DnaHash;
    constructor fn from_raw_32(ThirtyTwoHashBytes);
);

fixturator!(
    DhtOpHash;
    constructor fn from_raw_32(ThirtyTwoHashBytes);
);

fixturator!(
    with_vec 0 5;
    ActionHash;
    constructor fn from_raw_32(ThirtyTwoHashBytes);
);

fixturator!(
    NetIdHash;
    constructor fn from_raw_32(ThirtyTwoHashBytes);
);

fixturator!(
    WasmHash;
    constructor fn from_raw_32(ThirtyTwoHashBytes);
);

fixturator!(
    AnyDhtHash;
    constructor fn from_raw_32_and_type(ThirtyTwoHashBytes, HashTypeAnyDht);
);

fixturator!(
    AnyLinkableHash;
    constructor fn from_raw_32_and_type(ThirtyTwoHashBytes, HashTypeAnyLinkable);
);
