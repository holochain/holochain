//! Metadata types for use in wasm
use crate::commit::Commit;
use crate::commit::SignedActionHashed;
use crate::validate::ValidationStatus;
use crate::Entry;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
#[serde(tag = "type", content = "content")]
/// Return type for get_details calls.
/// ActionHash returns a Commit.
/// EntryHash returns an Entry.
pub enum Details {
    /// Variant asking for a specific commit
    Commit(CommitDetails),
    /// Variant asking for any information on data
    Entry(EntryDetails),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
/// A specific Commit with any deletes
/// This is all the metadata available for a commit.
pub struct CommitDetails {
    /// The specific commit.
    /// Either a Create or an Update.
    pub commit: Commit,
    /// The validation status of this commit.
    pub validation_status: ValidationStatus,
    /// Any [`Delete`](crate::action::Delete) on this commit.
    pub deletes: Vec<SignedActionHashed>,
    /// Any [`Update`](crate::action::Update) on this commit.
    pub updates: Vec<SignedActionHashed>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
/// An Entry with all it's metadata.
pub struct EntryDetails {
    /// The data
    pub entry: Entry,
    /// ## Create relationships.
    /// These are the actions that created this entry.
    /// They can be either a [`Create`](crate::action::Create) or an
    /// [`Update`](crate::action::Update) action
    /// where the `entry_hash` field is the hash of
    /// the above entry.
    ///
    /// You can make an [`Commit`] from any of these
    /// and the entry.
    pub actions: Vec<SignedActionHashed>,
    /// Rejected create relationships.
    /// These are also the actions that created this entry.
    /// but did not pass validation.
    pub rejected_actions: Vec<SignedActionHashed>,
    /// ## Delete relationships
    /// These are the deletes that have the
    /// `deletes_entry_address` set to the above Entry.
    pub deletes: Vec<SignedActionHashed>,
    /// ## Update relationships.
    /// These are the updates that have the
    /// `original_entry_address` set to the above Entry.
    /// ### Notes
    /// This is just the relationship and you will need call get
    /// if you want to get the new Entry (the entry on the `entry_hash` field).
    ///
    /// You **cannot** make an [Commit] from these actions
    /// and the above entry.
    pub updates: Vec<SignedActionHashed>,
    /// The status of this entry currently
    /// according to your view of the metadata
    pub entry_dht_status: EntryDhtStatus,
}

/// The status of an [Entry] in the Dht
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryDhtStatus {
    /// This [Entry] has active actions
    Live,
    /// This [Entry] has no actions that have not been deleted
    Dead,
    /// This [Entry] is awaiting validation
    Pending,
    /// This [Entry] has failed validation and will not be served by the DHT
    Rejected,
    /// This [Entry] has taken too long / too many resources to validate, so we gave up
    Abandoned,
    /// **not implemented** There has been a conflict when validating this [Entry]
    Conflict,
    /// **not implemented** The author has withdrawn their publication of this commit.
    Withdrawn,
    /// **not implemented** We have agreed to drop this [Entry] content from the system. Action can stay with no entry
    Purged,
}
