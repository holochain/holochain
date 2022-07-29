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
use crate::EntryDefIndex;
use crate::ZomeId;
use holo_hash::hash_type;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HashableContent;
use holo_hash::HashableContentBytes;
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

/// Helper trait for deserializing [`Entry`]s to the correct type.
///
/// This is implemented by the `hdk_entry_defs` proc_macro.
pub trait EntryTypesHelper: Sized {
    /// The error associated with this conversion.
    type Error;
    /// Check if the [`ZomeId`] and [`EntryDefIndex`] matches one of the
    /// `ZomeEntryTypesKey::from(Self::variant)` and if
    /// it does deserialize the [`Entry`] into that type.
    fn deserialize_from_type<Z, I>(
        zome_id: Z,
        entry_def_index: I,
        entry: &Entry,
    ) -> Result<Option<Self>, Self::Error>
    where
        Z: Into<ZomeId>,
        I: Into<EntryDefIndex>;
}

impl EntryTypesHelper for () {
    type Error = core::convert::Infallible;

    fn deserialize_from_type<Z, I>(
        _zome_id: Z,
        _entry_def_index: I,
        _entry: &Entry,
    ) -> Result<Option<Self>, Self::Error>
    where
        Z: Into<ZomeId>,
        I: Into<EntryDefIndex>,
    {
        Ok(Some(()))
    }
}

impl From<EntryHashed> for Entry {
    fn from(entry_hashed: EntryHashed) -> Self {
        entry_hashed.into_content()
    }
}

/// Structure holding the entry portion of a chain record.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

/// Zome input for must_get_valid_record.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MustGetValidRecordInput(pub ActionHash);

impl MustGetValidRecordInput {
    /// Constructor.
    pub fn new(action_hash: ActionHash) -> Self {
        Self(action_hash)
    }

    /// Consumes self for inner.
    pub fn into_inner(self) -> ActionHash {
        self.0
    }
}

/// Zome input for must_get_entry.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MustGetEntryInput(pub EntryHash);

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

/// Zome input for must_get_action.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct MustGetActionInput(pub ActionHash);

impl MustGetActionInput {
    /// Constructor.
    pub fn new(action_hash: ActionHash) -> Self {
        Self(action_hash)
    }

    /// Consumes self for inner.
    pub fn into_inner(self) -> ActionHash {
        self.0
    }
}
