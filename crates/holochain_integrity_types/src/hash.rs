use crate::prelude::*;
use holo_hash::ActionHash;
use holo_hash::EntryHash;
use holo_hash::ExternalHash;

/// 256 Bit generic hash.
pub struct Hash256Bits([u8; 32]);
crate::secure_primitive!(Hash256Bits, 32);

/// 512 Bit generic hash.
pub struct Hash512Bits([u8; 64]);
crate::secure_primitive!(Hash512Bits, 64);

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
/// Input to holochain hash function.
pub enum HashInput {
    /// Hash an Entry.
    Entry(Entry),
    /// Hash an action.
    Action(Action),
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
/// Output from the holochain hashing host function.
pub enum HashOutput {
    /// Hash of an [`Entry`].
    Entry(EntryHash),
    /// Hash of a [`Action`].
    Action(ActionHash),
    /// Hash of an external type.
    External(ExternalHash),
    /// Hash of bytes using Blake2b.
    Blake2B(#[serde(with = "serde_bytes")] Vec<u8>),
    /// Hash of bytes using SHA-256.
    Sha256(Hash256Bits),
    /// Hash of bytes using SHA-512.
    Sha512(Hash512Bits),
    /// Hash of bytes using Keccak256.
    Keccak256(Hash256Bits),
    /// Hash of bytes using NIST standard SHA3-256.
    Sha3256(Hash256Bits),
}
