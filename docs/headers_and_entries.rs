pub struct Address;
pub struct Signature;
pub struct PublicKey;
pub struct Timestamp;
pub struct DnaHash;
pub struct HeaderHash;
pub struct SerializedBytes;
pub struct EntryContentHash;
pub struct CapClaim;
pub struct CapGrant;
pub struct ZomePosition;

mod holo_hash_ext {
    pub struct Hash;
}

//======================= EndCompile Junk

pub struct Element(Signature, Header, Option<Entry>);

pub enum Header {
    // The first header in a chain (for the DNA) doesn't have a previous header
    Dna(header::Dna),
    LinkAdd(header::LinkAdd),
    LinkRemove(header::LinkRemove),
    ChainOpen(header::ChainOpen),
    ChainClose(header::ChainClose),
    EntryCreate(header::EntryCreate),
    EntryUpdate(header::EntryUpdate),
    ElementDelete(header::ElementDelete),
}

pub mod header {
    //! Holochain's header variations
    //!
    //! All header variations contain the fields `author` and `timestamp`.
    //! Furthermore, all variations besides pub struct `Dna` (which is the first header
    //! in a chain) contain the field `prev_header`.

    use super::*; // to get it to Compile

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

        pub base: Address, // Not Address, but HeaderHash or EntryContentHash or PublicKey
        pub target: Address, // Not Address, but HeaderHash or EntryContentHash or PublicKey
        pub tag: SerializedBytes,
        pub link_type: SerializedBytes,
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
        pub entry_hash: EntryContentHash,
    }

    pub struct EntryUpdate {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        pub replaces: Address, // not Address but EntryContentHash or HeaderHash ??

        pub entry_type: EntryType,
        pub entry_hash: EntryContentHash,
    }

    pub struct ElementDelete {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        /// Hash Address of the Element being deleted
        pub removes: Address, // not Address but EntryContentHash or HeaderHash ??
    }
}

impl Header {
    pub fn is_public() -> bool {
        unimplemented!()
    }
    pub fn author() -> PublicKey {
        unimplemented!()
    }
    pub fn timestamp() -> Timestamp {
        unimplemented!()
    }
    pub fn hash() -> holo_hash_ext::Hash {
        unimplemented!() // hash of header!!
    }
    pub fn prev_header(&self) -> Option<&HeaderHash> {
        Some(match self {
            Self::Dna(header::Dna { .. }) => return None,
            Self::LinkAdd(header::LinkAdd { prev_header, .. }) => prev_header,
            Self::LinkRemove(header::LinkRemove { prev_header, .. }) => prev_header,
            Self::ElementDelete(header::ElementDelete { prev_header, .. }) => prev_header,
            Self::ChainClose(header::ChainClose { prev_header, .. }) => prev_header,
            Self::ChainOpen(header::ChainOpen { prev_header, .. }) => prev_header,
            Self::EntryCreate(header::EntryCreate { prev_header, .. }) => prev_header,
            Self::EntryUpdate(header::EntryUpdate { prev_header, .. }) => prev_header,
        })
    }
}

pub enum Entry {
    CapClaim(CapClaim),
    CapGrant(CapGrant),
    AgentKey(PublicKey),
    // Stores the App's provided entry data
    App(AppEntry),
}

pub struct AppEntry {
    pub zome_id: ZomePosition,
    pub entry: Vec<u8>,
}

pub enum EntryType {
    AgentKey,
    // Stores the App's provided filtration data
    // FIXME: Change this if we are keeping Zomes
    App {
        zome_id: ZomePosition,
        app_entry_type: AppEntryType,
        is_public: bool,
    },
    CapClaim,
    CapGrant,
}

pub struct AppEntryType(Vec<u8>);
