//! Functions to generate standardized hashes of Holochain records and
//! arbitrary bytes.
//!
//! Holochain makes extensive use of hashes to address any content. It utilizes
//! [Blake2b](https://www.blake2.net/) as hashing algorithm. Holochain hashes
//! have a length of 39 bytes, made up of 3 bytes for identifying the hash, 32
//! bytes of digest and 4 location bytes. The complete scheme of a hash in byte
//! format is:
//!
//! ```text
//! <hash type code as varint><hash size in bytes><hash<location>>
//! ```
//!
//! The complete scheme of encoded hashes is:
//!
//! ```text
//! <encoding scheme><hash type code as varint><hash size in bytes><hash<location>>
//! ```
//!
//!
//! ## Example
//! This is an example of a public agent key hash, displayed as a byte array in
//! decimal notation:
//!
//! ```text
//! 132  32  36  39 218 126  34  87
//! 204 165 227 255  29 236 160  66
//! 221 163 168 112 215 187 143 152
//!  68   4  30 206 173 203 210 111
//! 103 207 124   2 107  67  33
//! ```
//!
//! ### Base64 encoding
//!
//! Since hashes have to be exchanged over the network, their bytes are encoded
//! before sending and decoded after reception, to avoid data corruption during
//! transport. To convert the hashes from a byte format to a transferrable
//! string, Holochain encodes them with the
//! [Base64 scheme](https://developer.mozilla.org/en-US/docs/Glossary/Base64).
//! Encoding the example public agent key in Base64 results in:
//!
//! ```text
//! hCAkJ9p+IlfMpeP/HeygQt2jqHDXu4+YRAQezq3L0m9nz3wCa0Mh
//! ```
//!
//! ### Self-identifying hash encoding
//!
//! Following the
//! [Multibase protocol](https://github.com/multiformats/multibase), hashes in
//! Holochain self-identify its encoding in Base64. All hashes in text format
//! are prefixed with a `u`, to identify them as
//! [`base64url`](https://github.com/multiformats/multibase/blob/master/multibase.csv#L23).
//! This encoding guarantees URL and filename safe Base64 strings through
//! replacing potentially unsafe characters like `+` and `/` by `-` and `_`
//! (see [RFC4648](https://datatracker.ietf.org/doc/html/rfc4648#section-5)).
//! The example public agent key becomes:
//!
//! ```text
//! uhCAkJ9p-IlfMpeP_HeygQt2jqHDXu4-YRAQezq3L0m9nz3wCa0Mh
//! ```
//!
//! > This only applies to Base64 encoded strings. Hashes in binary format
//! > **must not** be prefixed with `u`.
//!
//!
//! ### Self-identifying hash type and size
//!
//! Further self-identification of Holochain hashes is achieved by adhering to
//! the [Multihash protocol](https://github.com/multiformats/multihash). The
//! scheme it defines allows for including information on the semantic type of
//! hash and its length in Base64 encoded strings. Resulting hashes have the
//! following format:
//!
//! ```text
//! <hash type code as varint><hash size in bytes><hash>
//! ```
//!
//! Hashes in Holochain are 39 bytes long and comprise the hash type code, the
//! hash size in bytes and the hash. Coming back to the byte array
//! representation of the example agent pub key, the first 3 bytes are
//! `132 32 36`. In hexadecimal notation, it is written as `0x84 0x20 0x24`.
//!
//! Byte 1 and 2 are taken up by the hash type code as an
//! [unsigned varint](https://github.com/multiformats/unsigned-varint). Varint
//! is a serial encoding of an integer as a byte array of variable length.
//! When decoded to a regular integer, varint `132 32` equates to `4100`. This
//! and the other Multihash values employed for Holochain hashes meet several
//! criteria:
//!
//! * It encodes as more than one byte, as one byte entries are reserved in
//!   Multihash.
//! * An encoding consisting of two bytes plus the length byte makes three
//!   bytes, which always translates to 4 characters in Base64 encoding.
//! * The resulting Base64 encoding is supposed to be human-recognizable. `hC`
//!   was chosen in accordance with `holoChain`.
//!
//! Byte 3, which is `0x24` in hexadecimal and `36` in decimal notation,
//! reflects the hash size in bytes, meaning the **hashes are 36 bytes long**.
//!
//! ### Digest and DHT location
//!
//! The 36 bytes long hash consists of the actual digest of the hashed content
//! and the computed location of the hash within the distributed hash table
//! (DHT). The Blake2b algorithm used by Holochain produces hashes of 32 bytes
//! length.
//!
//! The final 4 bytes are location bytes. They are interpreted to identify
//! the position of an agent's arc, meaning the portion of the DHT that the
//! agent holds. Location bytes further serve as an integrity check of the hash
//! itself.
//!
//!
//! ## Valid Holochain hash types
//!
//! Here is a list of all valid hash types in Holochain, in hexadecimal,
//! decimal and Base64 notation and what they are used for:
//!
//! | hex      | decimal   | base64 | integer |  usage   |
//! | -------- | --------- | ------ | ------- | -------- |
//! | 84 20 24 | 132 32 36 | hCAk   | 4100    | Agent    |
//! | 84 21 24 | 132 33 36 | hCEk   | 4228    | Entry    |
//! | 84 22 24 | 132 34 36 | hCIk   | 4356    | Net ID   |
//! | 84 23 24 | 132 35 36 | hCMk   | 4484    |          |
//! | 84 24 24 | 132 36 36 | hCQk   | 4612    | DHT Op   |
//! | 84 25 24 | 132 37 36 | hCUk   | 4740    |          |
//! | 84 26 24 | 132 38 36 | hCYk   | 4868    |          |
//! | 84 27 24 | 132 39 36 | hCck   | 4996    |          |
//! | 84 28 24 | 132 40 36 | hCgk   | 5124    |          |
//! | 84 29 24 | 132 41 36 | hCkk   | 5252    | Action   |
//! | 84 2a 24 | 132 42 36 | hCok   | 5380    | WASM     |
//! | 84 2b 24 | 132 43 36 | hCsk   | 5508    |          |
//! | 84 2c 24 | 132 44 36 | hCwk   | 5636    |          |
//! | 84 2d 24 | 132 45 36 | hC0k   | 5764    | DNA      |
//! | 84 2e 24 | 132 46 36 | hC4k   | 5892    |          |
//! | 84 2f 24 | 132 47 36 | hC8k   | 6020    | External |
//!
//!
//! ### Breakdowns of example
//!
//! Breakdown of the example agent pub key as byte array in decimal notation:
//!
//! | type                   | length        | hash                                                                                                                   | dht location   |
//! | ---------------------- | ------------- | ---------------------------------------------------------------------------------------------------------------------- | -------------- |
//! | 132 32                 | 36            | 39 218 126 34 87 204 165 227 255 29 236 160 66 221 163 168 112 215 187 143 152 68 4 30 206 173 203 210 111 103 207 124 | 2 107 67 33    |
//! | public agent key       | 36 bytes long | Blake2b hash, 32 bytes long                                                                                            | u32: 558066434 |
//!
//!
//! Breakdown of the example agent pub key encoded as Base64:
//!
//! | Multibase encoding   | type + length                       | hash + dht location                                     |
//! | -------------------- | ----------------------------------- | ------------------------------------------------------- |
//! | u                    | hCAk                                | J9p-IlfMpeP_HeygQt2jqHDXu4-YRAQezq3L0m9nz3wCa0Mh        |
//! | base64url no padding | public agent key of 36 bytes length | Base64 encoding of Blake2b hash + location              |

use crate::prelude::*;

/// Hash anything that implements [`TryInto<Entry>`].
///
/// Hashes are typed in Holochain, e.g. [`ActionHash`] and [`EntryHash`] are different and yield different
/// bytes for a given value. This ensures correctness and allows type based dispatch in various
/// areas of the codebase.
///
/// Usually you want to hash a value that you want to reference on the DHT with [`must_get_entry`] etc. because
/// it represents some domain-specific data sourced externally or generated within the wasm.
/// [`ActionHash`] hashes are _always_ generated by the process of committing something to a local
/// chain. Every host function that commits an entry returns the new [`ActionHash`]. The [`ActionHash`] can
/// also be used with [`must_get_action`] etc. to retreive a _specific_ record from the DHT rather than the
/// oldest live record.
/// However there is no way to _generate_ an action hash directly from an action from inside wasm.
/// [`Record`] values (entry+action pairs returned by [`must_get_action`] etc.) contain prehashed action structs
/// called [`ActionHashed`], which is composed of a [`ActionHash`] alongside the "raw" [`Action`] value. Generally the pre-hashing is
/// more efficient than hashing actions ad-hoc as hashing always needs to be done at the database
/// layer, so we want to re-use that as much as possible.
/// The action hash can be extracted from the Record as `record.action_hashed().as_hash()`.
///
/// @todo is there any use-case that can't be satisfied by the `action_hashed` approach?
///
/// Anything that is annotated with #[hdk_entry( .. )] or entry_def!( .. ) implements this so is
/// compatible automatically.
///
/// [`hash_entry`] is "dumb" in that it doesn't check that the entry is defined, committed, on the DHT or
/// any other validation, it simply generates the hash for the serialized representation of
/// something in the same way that the DHT would.
///
/// It is strongly recommended that you use the [`hash_entry`] function to calculate hashes to avoid
/// inconsistencies between hashes in the wasm guest and the host.
/// For example, a lot of the crypto crates in rust compile to wasm so in theory could generate the
/// hash in the guest, but there is the potential that the serialization logic could be slightly
/// different, etc.
///
/// ```ignore
/// #[hdk_entry(id="foo")]
/// struct Foo;
///
/// let foo_hash = hash_entry(Foo)?;
/// ```
pub fn hash_entry<I, E>(input: I) -> ExternResult<EntryHash>
where
    Entry: TryFrom<I, Error = E>,
    WasmError: From<E>,
{
    match HDI.with(|h| h.borrow().hash(HashInput::Entry(Entry::try_from(input)?)))? {
        HashOutput::Entry(entry_hash) => Ok(entry_hash),
        _ => unreachable!(),
    }
}

/// Hash an `Action` into an `ActionHash`.
///
/// [`hash_entry`] has more of a discussion around different hash types and how
/// they are used within the HDI.
///
/// It is strongly recommended to use [`hash_action`] to calculate the hash rather than hand rolling an in-wasm solution.
/// Any inconsistencies in serialization or hash handling will result in dangling references to things due to a "corrupt" hash.
///
/// Note that usually relevant HDI functions return a [`ActionHashed`] or [`SignedActionHashed`] which already has associated methods to access the `ActionHash` of the inner `Action`.
/// In normal usage it is unlikely to be required to separately hash a [`Action`] like this.
pub fn hash_action(input: Action) -> ExternResult<ActionHash> {
    match HDI.with(|h| h.borrow().hash(HashInput::Action(input)))? {
        HashOutput::Action(action_hash) => Ok(action_hash),
        _ => unreachable!(),
    }
}

/// Hash arbitrary bytes using BLAKE2b.
/// This is the same algorithm used by holochain for typed hashes.
/// Notably the output hash length is configurable.
pub fn hash_blake2b(input: Vec<u8>, output_len: u8) -> ExternResult<Vec<u8>> {
    match HDI.with(|h| h.borrow().hash(HashInput::Blake2B(input, output_len)))? {
        HashOutput::Blake2B(vec) => Ok(vec),
        _ => unreachable!(),
    }
}

/// @todo - not implemented on the host
pub fn hash_sha256(input: Vec<u8>) -> ExternResult<Vec<u8>> {
    match HDI.with(|h| h.borrow().hash(HashInput::Sha256(input)))? {
        HashOutput::Sha256(hash) => Ok(hash.as_ref().to_vec()),
        _ => unreachable!(),
    }
}

/// @todo - not implemented on the host
pub fn hash_sha512(input: Vec<u8>) -> ExternResult<Vec<u8>> {
    match HDI.with(|h| h.borrow().hash(HashInput::Sha512(input)))? {
        HashOutput::Sha512(hash) => Ok(hash.as_ref().to_vec()),
        _ => unreachable!(),
    }
}

/// Hash arbitrary bytes using keccak256.
/// This is the same algorithm used by ethereum and other EVM compatible blockchains.
/// It is essentially the same as sha3 256 but with a minor difference in configuration
/// that is enough to generate different hash outputs.
pub fn hash_keccak256(input: Vec<u8>) -> ExternResult<Vec<u8>> {
    match HDI.with(|h| h.borrow().hash(HashInput::Keccak256(input)))? {
        HashOutput::Keccak256(hash) => Ok(hash.as_ref().to_vec()),
        _ => unreachable!(),
    }
}

/// Hash arbitrary bytes using SHA3 256.
/// This is the official NIST standard for 256 bit SHA3 hashes.
pub fn hash_sha3(input: Vec<u8>) -> ExternResult<Vec<u8>> {
    match HDI.with(|h| h.borrow().hash(HashInput::Sha3256(input)))? {
        HashOutput::Sha3256(hash) => Ok(hash.as_ref().to_vec()),
        _ => unreachable!(),
    }
}
