use hdi::prelude::LinkTypeFilterExt;
use holo_hash::{AgentPubKey, AnyLinkableHash};
use holochain_wasmer_guest::WasmError;
use holochain_zome_types::prelude::*;

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
            get_options: GetOptions::default(),
            tag_prefix: None,
            before: None,
            after: None,
            author: None,
        }))
    }

    /// Fetch links from network or local only.
    pub fn get_options(mut self, get_strategy: GetStrategy) -> Self {
        self.0.get_options.strategy = get_strategy;
        self
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

    /// Construct the result of the builder
    pub fn build(self) -> GetLinksInput {
        self.0
    }
}
