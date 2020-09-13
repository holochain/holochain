pub struct Address;
pub struct Signature;
pub struct PublicKey;
pub struct Timestamp;
pub struct DnaHash;
pub struct HeaderHash;
pub struct SerializedBytes;
pub struct EntryHash;
pub struct CapClaim;
pub struct CapGrant;
pub struct ZomePosition;

mod holo_hash {
    pub struct Hash;
}

//======================= EndCompile Junk

pub struct Element(Signature, Header, Option<Entry>);

pub enum Header {
    // The first header in a chain (for the DNA) doesn't have a previous header
    Dna(header::Dna),
    CreateLink(header::CreateLink),
    DeleteLink(header::DeleteLink),
    OpenChain(header::OpenChain),
    CloseChain(header::CloseChain),
    CreateEntry(header::CreateEntry),
    UpdateEntry(header::UpdateEntry),
    DeleteElement(header::DeleteElement),
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

    pub struct CreateLink {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        pub base: Address, // Not Address, but HeaderHash or EntryHash or PublicKey
        pub target: Address, // Not Address, but HeaderHash or EntryHash or PublicKey
        pub tag: SerializedBytes,
        pub link_type: SerializedBytes,
    }

    pub struct DeleteLink {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,
        /// The address of the `CreateLink` being reversed
        pub link_add_hash: Address, // not Address byt CreateLinkHash or maybe its HeaderHash?
    }

    pub struct OpenChain {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        pub prev_dna_hash: DnaHash,
    }

    pub struct CloseChain {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        pub new_dna_hash: DnaHash,
    }

    pub struct CreateEntry {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        pub entry_type: EntryType,
        pub entry_hash: EntryHash,
    }

    pub struct UpdateEntry {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        pub replaces: Address, // not Address but EntryHash or HeaderHash ??

        pub entry_type: EntryType,
        pub entry_hash: EntryHash,
    }

    pub struct DeleteElement {
        pub author: PublicKey,
        pub timestamp: Timestamp,
        pub prev_header: HeaderHash,

        /// Hash Address of the Element being deleted
        pub removes: Address, // not Address but EntryHash or HeaderHash ??
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
    pub fn hash() -> holo_hash::Hash {
        unimplemented!() // hash of header!!
    }
    pub fn prev_header(&self) -> Option<&HeaderHash> {
        Some(match self {
            Self::Dna(header::Dna { .. }) => return None,
            Self::CreateLink(header::CreateLink { prev_header, .. }) => prev_header,
            Self::DeleteLink(header::DeleteLink { prev_header, .. }) => prev_header,
            Self::DeleteElement(header::DeleteElement { prev_header, .. }) => prev_header,
            Self::CloseChain(header::CloseChain { prev_header, .. }) => prev_header,
            Self::OpenChain(header::OpenChain { prev_header, .. }) => prev_header,
            Self::CreateEntry(header::CreateEntry { prev_header, .. }) => prev_header,
            Self::UpdateEntry(header::UpdateEntry { prev_header, .. }) => prev_header,
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
