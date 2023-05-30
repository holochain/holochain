use hdi::prelude::LinkTypeFilterExt;
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash};
use holochain_wasmer_guest::WasmError;
use holochain_zome_types::{GetLinksInput, LinkTag, Timestamp};

/// A builder to streamline creating a `GetLinksInput`
#[derive(PartialEq, Clone, Debug)]
pub struct GetLinksInputBuilder(GetLinksInput);

impl GetLinksInputBuilder {
    /// Create a new `GetLinksInputBuilder` from the required fields for a `GetLinksInput`
    pub fn try_new(
        base_address: impl Into<AnyLinkableHash>,
        link_type: impl LinkTypeFilterExt,
    ) -> Result<Self, WasmError> {
        Ok(GetLinksInputBuilder(GetLinksInput {
            base_address: base_address.into(),
            link_type: link_type.try_into_filter()?,
            tag_prefix: None,
            before: None,
            after: None,
            author: None,
            batch_size: None,
            batch_index: None,
            previous_batch_end: None,
        }))
    }

    /// Filter for links with the given tag prefix.
    pub fn tag_prefix(mut self, tag_prefix: LinkTag) -> Self {
        self.0.tag_prefix = Some(tag_prefix);
        self
    }

    /// Filter for links created before `before`.
    pub fn before(mut self, before: Timestamp) -> Self {
        self.0.before = Some(before);
        self
    }

    /// Filter for links create after `after`.
    pub fn after(mut self, after: Timestamp) -> Self {
        self.0.after = Some(after);
        self
    }

    /// Filter for links created by this author.
    pub fn author(mut self, author: AgentPubKey) -> Self {
        self.0.author = Some(author);
        self
    }

    /// Set the size of the batch to fetch.
    pub fn batch_size(mut self, batch_size: usize) -> Self {
        self.0.batch_size = Some(batch_size);
        self
    }

    /// Set the 0-based batch index to get.
    pub fn batch_index(mut self, batch_index: usize) -> Self {
        self.0.batch_index = Some(batch_index);
        self
    }

    /// Set the action hash for the end of the previous batch.
    pub fn previous_batch_end(mut self, previous_batch_end: ActionHash) -> Self {
        self.0.previous_batch_end = Some(previous_batch_end);
        self
    }

    /// Construct the result of the builder
    pub fn build(self) -> GetLinksInput {
        self.0
    }
}
