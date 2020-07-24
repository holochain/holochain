#![allow(missing_docs)]

use crate::{
    encode::holo_dht_location_bytes, hash_type, AgentPubKey, AnyDhtHash, DhtOpHash, DnaHash,
    EntryContentHash, EntryHash, HeaderHash, NetIdHash, WasmHash,
};
use fixt::prelude::*;

pub type HashTypeEntry = hash_type::Entry;
pub type HashTypeAnyDht = hash_type::AnyDht;

// FIXME: why doesn't this work?
// failed to resolve: could not find `strum` in `{{root}}`
//
// fixturator!(
//     HashTypeEntry;
//     unit variants [ Agent Content ] empty Content;
// );

fixturator!(
    HashTypeEntry;
    curve Empty HashTypeEntry::Content;
    curve Unpredictable HashTypeEntry::Content;
    curve Predictable HashTypeEntry::Content;
);

fixturator!(
    HashTypeAnyDht;
    curve Empty HashTypeAnyDht::Header;
    curve Unpredictable HashTypeAnyDht::Header;
    curve Predictable HashTypeAnyDht::Header;
);

fixturator!(
    AgentPubKey;
    constructor fn from_raw_bytes(ThirtySixBytes);
);

fixturator!(
    EntryContentHash;
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
    EntryHash;
    constructor fn from_raw_bytes_and_type(ThirtySixBytes, HashTypeEntry);
);

fixturator!(
    AnyDhtHash;
    constructor fn from_raw_bytes_and_type(ThirtySixBytes, HashTypeAnyDht);
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
