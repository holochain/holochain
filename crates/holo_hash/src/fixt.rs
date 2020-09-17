#![allow(missing_docs)]

use crate::{
    encode::holo_dht_location_bytes, hash_type, AgentPubKey, AnyDhtHash, DhtOpHash, DnaHash,
    EntryHash, HeaderHash, NetIdHash, WasmHash,
};
use ::fixt::prelude::*;
use std::convert::TryFrom;

pub type HashTypeEntry = hash_type::Entry;
pub type HashTypeAnyDht = hash_type::AnyDht;

// TODO: use strum to do this:
//
// fixturator!(
//     HashTypeEntry;
//     unit variants [ Agent Content ] empty Content;
// );

fixturator!(
    HashTypeAnyDht;
    curve Empty HashTypeAnyDht::Header;
    curve Unpredictable HashTypeAnyDht::Header;
    curve Predictable HashTypeAnyDht::Header;
);

/// A type alias for a Vec<u8> whose fixturator is expected to only return
/// a Vec of length 36
pub type ThirtySixBytes = Vec<u8>;

// Simply generate "bytes" which is a Vec<u8> of 36 bytes
fixturator!(
    ThirtySixBytes,
    append_location([0; 32].to_vec()),
    {
        let mut rng = rand::thread_rng();
        let mut u8_fixturator = U8Fixturator::new(Unpredictable);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        append_location(bytes)
    },
    {
        let mut u8_fixturator = U8Fixturator::new_indexed(Predictable, self.0.index);
        let mut bytes = vec![];
        for _ in 0..32 {
            bytes.push(u8_fixturator.next().unwrap());
        }
        self.0.index += 1;
        append_location(bytes)
    }
);

fn append_location(mut base: Vec<u8>) -> Vec<u8> {
    let mut loc_bytes = holo_dht_location_bytes(&base);
    base.append(&mut loc_bytes);
    base
}

fixturator!(
    AgentPubKey;
    curve Empty AgentPubKey::from_raw_bytes(ThirtySixBytesFixturator::new_indexed(Empty, self.0.index).next().unwrap());
    curve Unpredictable AgentPubKey::from_raw_bytes(ThirtySixBytesFixturator::new_indexed(Unpredictable, self.0.index).next().unwrap());
    curve Predictable {
        // these agent keys match what the mock keystore spits out for the first two agents
        // don't mess with this unless you also update the keystore!!!
        let agents = vec![
            AgentPubKey::try_from("uhCAkmrkoAHPVf_eufG7eC5fm6QKrW5pPMoktvG5LOC0SnJ4vV1Uv")
            .unwrap(),
            AgentPubKey::try_from("uhCAke1j8Z2a-_min0h0pGuEMcYlo_V1l1mt9OtBuywKmHlg4L_R-")
                .unwrap(),
        ];
        agents[self.0.index % agents.len()].clone()
    };
);

fixturator!(
    EntryHash;
    constructor fn from_raw_bytes(ThirtySixBytes);
);

fixturator!(
    DnaHash;
    constructor fn from_raw_bytes(ThirtySixBytes);
);

fixturator!(
    DhtOpHash;
    constructor fn from_raw_bytes(ThirtySixBytes);
);

fixturator!(
    HeaderHash;
    constructor fn from_raw_bytes(ThirtySixBytes);
);

fixturator!(
    NetIdHash;
    constructor fn from_raw_bytes(ThirtySixBytes);
);

fixturator!(
    WasmHash;
    constructor fn from_raw_bytes(ThirtySixBytes);
);

fixturator!(
    AnyDhtHash;
    constructor fn from_raw_bytes_and_type(ThirtySixBytes, HashTypeAnyDht);
);
