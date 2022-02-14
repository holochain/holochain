//! Metadata types for use in wasm
use crate::element::Element;
use crate::element::SignedHeaderHashed;
use crate::validate::ValidationStatus;
use crate::Entry;
use holochain_serialized_bytes::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
#[serde(tag = "type", content = "content")]
/// Return type for get_details calls.
/// HeaderHash returns an Element.
/// EntryHash returns an Entry.
pub enum Details {
    /// Variant holding for a specific element. Returned when [`get_details`] was passed a header hash.
    Element(ElementDetails),
    /// Variant holding all information on element.   Returned when [`get_details`] was passed an entry hash.
    Entry(EntryDetails),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
/// A specific Element with any deletes
/// This is all the metadata available for an element.
pub struct ElementDetails {
    /// The specific element.
    /// Either a Create or an Update.
    pub element: Element,
    /// The validation status of this element.
    pub validation_status: ValidationStatus,
    /// Any [Delete] on this element.
    pub deletes: Vec<SignedHeaderHashed>,
    /// Any [Update] on this element.
    pub updates: Vec<SignedHeaderHashed>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SerializedBytes)]
/// An Entry with all it's metadata.
pub struct EntryDetails {
    /// The data
    pub entry: Entry,
    /// ## Create relationships.
    /// These are the headers that created this entry.
    /// They can be either a [Create] or an [Update] header
    /// where the `entry_hash` field is the hash of
    /// the above entry.
    ///
    /// You can make an [Element] from any of these
    /// and the entry.
    pub headers: Vec<SignedHeaderHashed>,
    /// Rejected create relationships.
    /// These are also the headers that created this entry.
    /// but did not pass validation.
    pub rejected_headers: Vec<SignedHeaderHashed>,
    /// ## Delete relationships
    /// These are the deletes that have the
    /// `deletes_entry_address` set to the above Entry.
    pub deletes: Vec<SignedHeaderHashed>,
    /// ## Update relationships.
    /// These are the updates that have the
    /// `original_entry_address` set to the above Entry.
    /// ### Notes
    /// This is just the relationship and you will need call get
    /// if you want to get the new Entry (the entry on the `entry_hash` field).
    ///
    /// You **cannot** make an [Element] from these headers
    /// and the above entry.
    pub updates: Vec<SignedHeaderHashed>,
    /// The status of this entry currently
    /// according to your view of the metadata
    pub entry_dht_status: EntryDhtStatus,
}

/// The status of an [Entry] in the Dht
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryDhtStatus {
    /// This [Entry] has active headers
    Live,
    /// This [Entry] has no headers that have not been deleted
    Dead,
    /// This [Entry] is awaiting validation
    Pending,
    /// This [Entry] has failed validation and will not be served by the DHT
    Rejected,
    /// This [Entry] has taken too long / too many resources to validate, so we gave up
    Abandoned,
    /// **not implemented** There has been a conflict when validating this [Entry]
    Conflict,
    /// **not implemented** The author has withdrawn their publication of this element.
    Withdrawn,
    /// **not implemented** We have agreed to drop this [Entry] content from the system. Header can stay with no entry
    Purged,
}
