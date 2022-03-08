//! # Dht Operational Transforms

use crate::{
    AppEntryType, Create, CreateLink, Delete, DeleteLink, Element, Entry, EntryType, Header,
    HeaderRef, SignedHashed, SignedHeaderHashed, Update,
};
use holo_hash::{AgentPubKey, EntryHash, HashableContent, HeaderHash};
use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// # Dht Operational Transforms
/// These are the operational transformations that can be applied to Holochain data.
/// Every [`Header`] produces a set of operations.
/// These operations are each sent to an authority for validation.
///
/// ## Producing Operations
/// The following is a list of the operations that can be produced by each [`Header`]:
/// - Every [`Header`] produces a [`Op::RegisterAgentActivity`] and a [`Op::StoreElement`].
/// - [`Header::Create`] also produces a [`Op::StoreEntry`].
/// - [`Header::Update`] also produces a [`Op::StoreEntry`] and a [`Op::RegisterUpdate`].
/// - [`Header::Delete`] also produces a [`Op::RegisterDelete`].
/// - [`Header::CreateLink`] also produces a [`Op::RegisterCreateLink`].
/// - [`Header::DeleteLink`] also produces a [`Op::RegisterDeleteLink`].
///
/// ## Authorities
/// There are three types of authorities in Holochain:
///
/// #### The Header Authority
/// This set of authorities receives the [`Op::StoreElement`].
/// This is where you can implement your own logic for checking
/// that it is valid to store any of the [`Header`] variants
/// according to your own applications rules.
///
/// #### The Entry Authority
/// This set of authorities receives the [`Op::StoreEntry`].
/// This is where you can implement your own logic for checking
/// that it is valid to store an [`Entry`].
/// You can think of this as the "Create" from the CRUD acronym.
///
/// ##### Metadata
/// The entry authority is also responsible for storing the metadata for each entry.
/// They receive the [`Op::RegisterUpdate`] and [`Op::RegisterDelete`].
/// This is where you can implement your own logic for checking that it is valid to
/// update or delete any of the [`Entry`] types defined in your application.
/// You can think of this as the "Update" and "Delete" from the CRUD acronym.
///
/// They receive the [`Op::RegisterCreateLink`] and [`Op::RegisterDeleteLink`].
/// This is where you can implement your own logic for checking that it is valid to
/// place a link on a base [`Entry`].
///
/// #### The Chain Authority
/// This set of authorities receives the [`Op::RegisterAgentActivity`].
/// This is where you can implement your own logic for checking that it is valid to
/// add a new [`Header`] to an agent source chain.
/// You are not validating the individual element but the entire agents source chain.
///
/// ##### Author
/// When authoring a new [`Header`] to your source chain, the
/// validation will be run from the perspective of every authority.
///
/// ##### A note on metadata for the Header authority.
/// Technically speaking the Header authority also receives and validates the
/// [`Op::RegisterUpdate`] and [`Op::RegisterDelete`] but they run the same callback
/// as the Entry authority because it would be inconsistent to have two separate
/// validation outcomes for these ops.
///
/// ## Running Validation
/// When the `fn validate(op: Op) -> ExternResult<ValidateCallbackResult>` is called
/// it will be passed the operation variant for the authority that is
/// actually running the validation.
///
/// For example the entry authority will be passed the [`Op::StoreEntry`] operation.
/// The operational transforms that can are applied to Holochain data.
/// Operations beginning with `Store` are concerned with creating and
/// storing data.
/// Operations beginning with `Register` are concerned with registering
/// metadata about the data.
pub enum Op {
    /// Stores a new [`Element`] in the DHT.
    /// This is the act of creating a new [`Header`]
    /// and publishing it to the DHT.
    /// Note that not all [`Header`]s contain an [`Entry`].
    StoreElement {
        /// The [`Element`] to store.
        element: Element,
    },
    /// Stores a new [`Entry`] in the DHT.
    /// This is the act of creating a either a [`Header::Create`] or
    /// a [`Header::Update`] and publishing it to the DHT.
    /// These headers create a new instance of an [`Entry`].
    StoreEntry {
        /// The signed and hashed [`EntryCreationHeader`] that creates
        /// a new instance of the [`Entry`].
        header: SignedHashed<EntryCreationHeader>,
        /// The new [`Entry`] to store.
        entry: Entry,
    },
    /// Registers an update from an instance of an [`Entry`] in the DHT.
    /// This is the act of creating a [`Header::Update`] and
    /// publishing it to the DHT.
    /// Note that the [`Header::Update`] stores an new instance
    /// of an [`Entry`] and registers it as an update to the original [`Entry`].
    /// This operation is only concerned with registering the update.
    RegisterUpdate {
        /// The signed and hashed [`Header::Update`] that registers the update.
        update: SignedHashed<Update>,
        /// The new [`Entry`] that is being updated to.
        new_entry: Entry,
        /// The original [`EntryCreationHeader`] that created
        /// the original [`Entry`].
        /// Note that the update points to a specific instance of the
        /// of the original [`Entry`].
        original_header: EntryCreationHeader,
        /// The original [`Entry`] that is being updated from.
        original_entry: Entry,
    },
    /// Registers a deletion of an instance of an [`Entry`] in the DHT.
    /// This is the act of creating a [`Header::Delete`] and
    /// publishing it to the DHT.
    RegisterDelete {
        /// The signed and hashed [`Header::Delete`] that registers the deletion.
        delete: SignedHashed<Delete>,
        /// The original [`EntryCreationHeader`] that created
        /// the original [`Entry`].
        original_header: EntryCreationHeader,
        /// The original [`Entry`] that is being deleted.
        original_entry: Entry,
    },
    /// Registers a new [`Header`] on an agent source chain.
    /// This is the act of creating any [`Header`] and
    /// publishing it to the DHT.
    RegisterAgentActivity {
        /// The signed and hashed [`Header`] that is being registered.
        header: SignedHeaderHashed,
    },
    /// Registers a link between two [`Entry`]s.
    /// This is the act of creating a [`Header::CreateLink`] and
    /// publishing it to the DHT.
    /// The authority is the entry authority for the base [`Entry`].
    RegisterCreateLink {
        /// The signed and hashed [`Header::CreateLink`] that registers the link.
        create_link: SignedHashed<CreateLink>,
        /// The base [`Entry`] that is being linked from.
        base: Entry,
        /// The target [`Entry`] that is being linked to.
        target: Entry,
    },
    /// Deletes a link between two [`Entry`]s.
    /// This is the act of creating a [`Header::DeleteLink`] and
    /// publishing it to the DHT.
    /// The delete always references a specific [`Header::CreateLink`].
    RegisterDeleteLink {
        /// The signed and hashed [`Header::DeleteLink`] that registers the deletion.
        delete_link: SignedHashed<DeleteLink>,
        /// The link that is being deleted.
        create_link: CreateLink,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
#[cfg_attr(feature = "test_utils", derive(arbitrary::Arbitrary))]
/// Either a [`Header::Create`] or a [`Header::Update`].
/// These headers both create a new instance of an [`Entry`].
pub enum EntryCreationHeader {
    /// A [`Header::Create`] that creates a new instance of an [`Entry`].
    Create(Create),
    /// A [`Header::Update`] that creates a new instance of an [`Entry`].
    Update(Update),
}

impl EntryCreationHeader {
    /// The author of this header.
    pub fn author(&self) -> &AgentPubKey {
        match self {
            EntryCreationHeader::Create(Create { author, .. })
            | EntryCreationHeader::Update(Update { author, .. }) => author,
        }
    }
    /// The [`Timestamp`] for this header.
    pub fn timestamp(&self) -> &Timestamp {
        match self {
            EntryCreationHeader::Create(Create { timestamp, .. })
            | EntryCreationHeader::Update(Update { timestamp, .. }) => timestamp,
        }
    }
    /// The header sequence number of this header.
    pub fn header_seq(&self) -> &u32 {
        match self {
            EntryCreationHeader::Create(Create { header_seq, .. })
            | EntryCreationHeader::Update(Update { header_seq, .. }) => header_seq,
        }
    }
    /// The previous [`HeaderHash`] of the previous header in the source chain.
    pub fn prev_header(&self) -> &HeaderHash {
        match self {
            EntryCreationHeader::Create(Create { prev_header, .. })
            | EntryCreationHeader::Update(Update { prev_header, .. }) => prev_header,
        }
    }
    /// The [`EntryType`] of the [`Entry`] being created.
    pub fn entry_type(&self) -> &EntryType {
        match self {
            EntryCreationHeader::Create(Create { entry_type, .. })
            | EntryCreationHeader::Update(Update { entry_type, .. }) => entry_type,
        }
    }
    /// The [`EntryHash`] of the [`Entry`] being created.
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            EntryCreationHeader::Create(Create { entry_hash, .. })
            | EntryCreationHeader::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }
    /// The [`AppEntryType`] of the [`Entry`] being created if it
    /// is an application defined [`Entry`].
    pub fn app_entry_type(&self) -> Option<&AppEntryType> {
        match self.entry_type() {
            EntryType::App(app_entry_type) => Some(app_entry_type),
            _ => None,
        }
    }

    /// Returns `true` if this header creates an [`EntryType::AgentPubKey`] [`Entry`].
    pub fn is_agent_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::AgentPubKey)
    }

    /// Returns `true` if this header creates an [`EntryType::CapClaim`] [`Entry`].
    pub fn is_cap_claim_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::CapClaim)
    }

    /// Returns `true` if this header creates an [`EntryType::CapGrant`] [`Entry`].
    pub fn is_cap_grant_entry_type(&self) -> bool {
        matches!(self.entry_type(), EntryType::CapGrant)
    }
}

/// Allows a [`EntryCreationHeader`] to hash the same bytes as
/// the equivalent [`Header`] variant without needing to clone the header.
impl HashableContent for EntryCreationHeader {
    type HashType = holo_hash::hash_type::Header;

    fn hash_type(&self) -> Self::HashType {
        use holo_hash::PrimitiveHashType;
        holo_hash::hash_type::Header::new()
    }

    fn hashable_content(&self) -> holo_hash::HashableContentBytes {
        let h = match self {
            EntryCreationHeader::Create(create) => HeaderRef::Create(create),
            EntryCreationHeader::Update(update) => HeaderRef::Update(update),
        };
        let sb = SerializedBytes::from(UnsafeBytes::from(
            holochain_serialized_bytes::encode(&h).expect("Could not serialize HashableContent"),
        ));
        holo_hash::HashableContentBytes::Content(sb)
    }
}

impl From<EntryCreationHeader> for Header {
    fn from(e: EntryCreationHeader) -> Self {
        match e {
            EntryCreationHeader::Create(c) => Header::Create(c),
            EntryCreationHeader::Update(u) => Header::Update(u),
        }
    }
}

impl From<Create> for EntryCreationHeader {
    fn from(c: Create) -> Self {
        EntryCreationHeader::Create(c)
    }
}

impl From<Update> for EntryCreationHeader {
    fn from(u: Update) -> Self {
        EntryCreationHeader::Update(u)
    }
}

impl TryFrom<Header> for EntryCreationHeader {
    type Error = crate::WrongHeaderError;
    fn try_from(value: Header) -> Result<Self, Self::Error> {
        match value {
            Header::Create(h) => Ok(EntryCreationHeader::Create(h)),
            Header::Update(h) => Ok(EntryCreationHeader::Update(h)),
            _ => Err(crate::WrongHeaderError(format!("{:?}", value))),
        }
    }
}
