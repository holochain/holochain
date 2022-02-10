use holo_hash::HeaderHash;
use holo_hash::EntryHash;
use crate::prelude::*;

pub struct Hash256Bits([u8; 32]);
crate::secure_primitive!(Hash256Bits, 32);
pub struct Hash512Bits([u8; 64]);
crate::secure_primitive!(Hash512Bits, 64);

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HashInput {
    /// Hash an Entry.
    Entry(Entry),
    /// Hash a Header.
    Header(Header),
    /// Blake2b is the native Holochain hashing algorithm and compatible with
    /// e.g. Polkadot and Zcash.
    /// Second value is the output length of the hash in bytes.
    Blake2B(#[serde(with = "serde_bytes")] Vec<u8>, u8),
    /// 256 bit SHA-2 a.k.a. SHA-256 used by Bitcoin, IPFS, etc.
    Sha256(#[serde(with = "serde_bytes")] Vec<u8>),
    /// 512 bit SHA-2 a.k.a. SHA-512.
    Sha512(#[serde(with = "serde_bytes")] Vec<u8>),
    /// Keccak256 is the variant of SHA3 used by the Ethereum Virtual Machine.
    /// It is slightly different to the NIST standard SHA3-256.
    /// (i.e. the resulting hashes are completely different)
    Keccak256(#[serde(with = "serde_bytes")] Vec<u8>),
    /// NIST standard SHA3-256.
    Sha3256(#[serde(with = "serde_bytes")] Vec<u8>),
}

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub enum HashOutput {
    Entry(EntryHash),
    Header(HeaderHash),
    Blake2B(#[serde(with = "serde_bytes")] Vec<u8>),
    Sha256(Hash256Bits),
    Sha512(Hash512Bits),
    Keccak256(Hash256Bits),
    Sha3256(Hash256Bits),
}