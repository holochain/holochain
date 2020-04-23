pub struct Address;
pub struct Signature;
pub struct PublicKey;
pub struct Timestamp;
pub struct DnaHash;
pub struct HeaderHash;
pub struct SerializedBytes;
pub struct EntryHash;
pub struct CapTokenClaim;
pub struct CapTokenGrant;
pub struct ZomeId;

use crate::{
    entry::{Entry, EntryAddress},
    prelude::*,
};

/// Holochain's header variations
///
/// All header variations contain the fields `author` and `timestamp`.
/// Furthermore, all variations besides pub struct `Dna` (which is the first header
/// in a chain) contain the field `prev_header`.

pub struct Dna {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    // No previous header, because DNA is always first chain entry

    pub hash: DnaHash,
}

pub struct LinkAdd {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub base: Address,   // Not Address, but HeaderHash or EntryHash or PublicKey
    pub target: Address, // Not Address, but HeaderHash or EntryHash or PublicKey
    pub tag: SerializedBytes,
    pub link_type: SerializedBytes
}

pub struct LinkRemove {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,
    /// The address of the `LinkAdd` being reversed
    pub link_add_hash: Address, // not Address byt LinkAddHash or maybe its HeaderHash?
}

pub struct ChainOpen {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub prev_dna_hash: DnaHash,
}

pub struct ChainClose {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub new_dna_hash: DnaHash,
}

pub struct EntryCreate {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

pub struct EntryUpdate {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    pub replaces: Address, // not Address but EntryHash or HeaderHash ??

    pub entry_type: EntryType,
    pub entry_hash: EntryHash,
}

pub struct EntryDelete {
    pub author: PublicKey,
    pub timestamp: Timestamp,
    pub prev_header: HeaderHash,

    /// Hash Address of the Element being deleted
    pub removes: Address, // not Address but EntryHash or HeaderHash ??
}

pub enum EntryType {
    AgentKey,
    // Stores the App's provided filtration data
    // FIXME: Change this if we are keeping Zomes
    App {
        zome_id: ZomeId,
        app_entry_type: AppEntryType,
        is_public: bool,
    },
    CapTokenClaim,
    CapTokenGrant,
}


pub struct AppEntryType(Vec<u8>);
