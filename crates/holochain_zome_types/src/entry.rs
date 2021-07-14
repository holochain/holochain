//! An Entry is a unit of data in a Holochain Source Chain.
//!
//! This module contains all the necessary definitions for Entry, which broadly speaking
//! refers to any data which will be written into the ContentAddressableStorage, or the EntityAttributeValueStorage.
//! It defines serialization behaviour for entries. Here you can find the complete list of
//! entry_types, and special entries, like deletion_entry and cap_entry.

use crate::capability::CapClaim;
use crate::capability::CapGrant;
use crate::capability::ZomeCallCapGrant;
use crate::countersigning::CounterSigningSessionData;
use holo_hash::hash_type;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HashableContent;
use holo_hash::HashableContentBytes;
use holo_hash::HeaderHash;
use holochain_serialized_bytes::prelude::*;

mod app_entry_bytes;
mod error;
pub use app_entry_bytes::*;
pub use error::*;

/// Entries larger than this number of bytes cannot be created
pub const ENTRY_SIZE_LIMIT: usize = 16 * 1000 * 1000; // 16MiB

/// The data type written to the source chain when explicitly granting a capability.
/// NB: this is not simply `CapGrant`, because the `CapGrant::ChainAuthor`
/// grant is already implied by `Entry::Agent`, so that should not be committed
/// to a chain. This is a type alias because if we add other capability types
/// in the future, we may want to include them
pub type CapGrantEntry = ZomeCallCapGrant;

/// The data type written to the source chain to denote a capability claim
pub type CapClaimEntry = CapClaim;

/// An Entry paired with its EntryHash
pub type EntryHashed = holo_hash::HoloHashed<Entry>;

impl From<EntryHashed> for Entry {
    fn from(entry_hashed: EntryHashed) -> Self {
        entry_hashed.into_content()
    }
}

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
/// Options for controlling how get works
pub struct GetOptions {
    /// If this is true the get call will wait for
    /// the latest data before returning.
    /// If it is false you will get whatever is locally
    /// available on this conductor.
    pub strategy: GetStrategy,
}

impl GetOptions {
    /// This will get you the content
    /// with latest metadata if it can
    /// otherwise it will fallback to what
    /// you have cached locally.
    ///
    /// This call is guaranteed to not go to
    /// the network if you are an authority
    /// for this hash.
    pub fn latest() -> Self {
        Self {
            strategy: GetStrategy::Latest,
        }
    }
    /// Gets the content but does not
    /// try to get the latest metadata.
    /// This will save a network call if the
    /// entry is local (cached, authored or integrated).
    ///
    /// This will fallback to the network if the content
    /// is not found locally
    pub fn content() -> Self {
        Self {
            strategy: GetStrategy::Content,
        }
    }
}

impl Default for GetOptions {
    fn default() -> Self {
        Self::latest()
    }
}

#[derive(PartialEq, Debug, Clone, Copy, Serialize, Deserialize)]
/// Describes the get call and what information
/// the caller is concerned about.
/// This helps the subconscious avoid unnecessary network calls.
pub enum GetStrategy {
    /// Will try to get the latest metadata but fallback
    /// to the cache if none is found.
    /// Does not go to the network if you are an authority for the data.
    Latest,
    /// Will try to get the content locally but go
    /// to the network if it is not found.
    /// Does not go to the network if you are an authority for the data.
    Content,
}

/// Structure holding the entry portion of a chain element.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[serde(tag = "entry_type", content = "entry")]
pub enum Entry {
    /// The `Agent` system entry, the third entry of every source chain,
    /// which grants authoring capability for this agent.
    Agent(AgentPubKey),
    /// The application entry data for entries that aren't system created entries
    App(AppEntryBytes),
    /// Application entry data for entries that need countersigning to move forward multiple chains together.
    CounterSign(Box<CounterSigningSessionData>, AppEntryBytes),
    /// The capability claim system entry which allows committing a granted permission
    /// for later use
    CapClaim(CapClaimEntry),
    /// The capability grant system entry which allows granting of application defined
    /// capabilities
    CapGrant(CapGrantEntry),
}

impl Entry {
    /// If this entry represents a capability grant, return a `CapGrant`.
    pub fn as_cap_grant(&self) -> Option<CapGrant> {
        match self {
            Entry::Agent(key) => Some(CapGrant::ChainAuthor(key.clone())),
            Entry::CapGrant(data) => Some(CapGrant::RemoteAgent(data.clone())),
            _ => None,
        }
    }

    /// If this entry represents a capability claim, return a `CapClaim`.
    pub fn as_cap_claim(&self) -> Option<&CapClaim> {
        match self {
            Entry::CapClaim(claim) => Some(claim),
            _ => None,
        }
    }

    /// Create an Entry::App from SerializedBytes
    pub fn app(sb: SerializedBytes) -> Result<Self, EntryError> {
        Ok(Entry::App(AppEntryBytes::try_from(sb)?))
    }

    /// Create an Entry::App from SerializedBytes
    pub fn app_fancy<
        E: Into<EntryError>,
        SB: TryInto<SerializedBytes, Error = SerializedBytesError>,
    >(
        sb: SB,
    ) -> Result<Self, EntryError> {
        Ok(Entry::App(AppEntryBytes::try_from(sb.try_into()?)?))
    }
}

impl HashableContent for Entry {
    type HashType = hash_type::Entry;

    fn hash_type(&self) -> Self::HashType {
        hash_type::Entry
    }

    fn hashable_content(&self) -> HashableContentBytes {
        match self {
            Entry::Agent(agent_pubkey) => {
                // We must retype this AgentPubKey as an EntryHash so that the
                // prefix bytes match the Entry prefix
                HashableContentBytes::Prehashed39(
                    agent_pubkey
                        .clone()
                        .retype(holo_hash::hash_type::Entry)
                        .into_inner(),
                )
            }
            entry => HashableContentBytes::Content(
                entry
                    .try_into()
                    .expect("Could not serialize HashableContent"),
            ),
        }
    }
}

/// Data to create an entry.
#[derive(PartialEq, Clone, Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct EntryWithDefId {
    entry_def_id: crate::entry_def::EntryDefId,
    entry: crate::entry::Entry,
}

impl EntryWithDefId {
    /// Constructor.
    pub fn new(entry_def_id: crate::entry_def::EntryDefId, entry: crate::entry::Entry) -> Self {
        Self {
            entry_def_id,
            entry,
        }
    }
}

impl AsRef<crate::Entry> for EntryWithDefId {
    fn as_ref(&self) -> &crate::Entry {
        &self.entry
    }
}

impl AsRef<crate::EntryDefId> for EntryWithDefId {
    fn as_ref(&self) -> &crate::EntryDefId {
        &self.entry_def_id
    }
}

/// Zome IO for get and get_details calls.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct GetInput {
    /// Any DHT hash to pass to get or get_details.
    pub any_dht_hash: holo_hash::AnyDhtHash,
    /// Options for the call.
    pub get_options: crate::entry::GetOptions,
}

impl GetInput {
    /// Constructor.
    pub fn new(any_dht_hash: holo_hash::AnyDhtHash, get_options: crate::entry::GetOptions) -> Self {
        Self {
            any_dht_hash,
            get_options,
        }
    }
}

/// Zome IO for must_get_valid_element.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MustGetValidElementInput(HeaderHash);

impl MustGetValidElementInput {
    /// Constructor.
    pub fn new(header_hash: HeaderHash) -> Self {
        Self(header_hash)
    }

    /// Consumes self for inner.
    pub fn into_inner(self) -> HeaderHash {
        self.0
    }
}

/// Zome IO for must_get_entry.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MustGetEntryInput(EntryHash);

impl MustGetEntryInput {
    /// Constructor.
    pub fn new(entry_hash: EntryHash) -> Self {
        Self(entry_hash)
    }

    /// Consumes self for inner.
    pub fn into_inner(self) -> EntryHash {
        self.0
    }
}

/// Zome IO for must_get_header.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MustGetHeaderInput(HeaderHash);

impl MustGetHeaderInput {
    /// Constructor.
    pub fn new(header_hash: HeaderHash) -> Self {
        Self(header_hash)
    }

    /// Consumes self for inner.
    pub fn into_inner(self) -> HeaderHash {
        self.0
    }
}

/// Zome IO inner for update.
#[derive(PartialEq, Debug, Deserialize, Serialize, Clone)]
pub struct UpdateInput {
    /// Header of the element being updated.
    pub original_header_address: holo_hash::HeaderHash,
    /// Value of the update.
    pub entry_with_def_id: EntryWithDefId,
}

impl UpdateInput {
    /// Constructor.
    pub fn new(
        original_header_address: holo_hash::HeaderHash,
        entry_with_def_id: EntryWithDefId,
    ) -> Self {
        Self {
            original_header_address,
            entry_with_def_id,
        }
    }
}
