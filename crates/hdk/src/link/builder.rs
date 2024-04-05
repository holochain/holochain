use hdi::prelude::LinkTypeFilterExt;
use holo_hash::{AgentPubKey, AnyLinkableHash};
use holochain_wasmer_guest::WasmError;
use holochain_zome_types::prelude::*;

/// A builder to streamline creating a `GetLinksInput`.
///
/// Example: Get links of any time from a given base address.
/// ```rust,no_run
/// use hdk::prelude::*;
///
/// # fn main() -> ExternResult<()> {
///     let my_base = ActionHash::from_raw_36(vec![0; 36]); // Some base address, this is a dummy address created for the example!
///     let links = get_links(GetLinksInputBuilder::try_new(my_base, ..)?.build())?;
/// #   Ok(())
/// # }
/// ```
///
/// Example: Get links of a specific type from a given base address.
/// ```rust,no_run
/// use hdk::prelude::*;
///
/// #[hdk_link_types]
/// pub enum LinkTypes {
///     Example,
/// }
///
/// # fn main() -> ExternResult<()> {
///     let my_base = ActionHash::from_raw_36(vec![0; 36]); // Some base address, this is a dummy address created for the example!
///     let links = get_links(GetLinksInputBuilder::try_new(my_base, LinkTypes::Example)?.build())?;
/// #   Ok(())
/// # }
/// ```
///
/// You can add additional filters using the functions defined on the builder.
/// For example, to only fetch links that are available locally, without going to the network:
/// ```rust,no_run
/// use hdk::prelude::*;
///
/// # fn main() -> ExternResult<()> {
///     let my_base = ActionHash::from_raw_36(vec![0; 36]); // Some base address, this is a dummy address created for the example!
///     let links = get_links(GetLinksInputBuilder::try_new(my_base, ..)?.get_options(GetStrategy::Local).build())?;
/// #   Ok(())
/// # }
#[derive(PartialEq, Clone, Debug)]
pub struct GetLinksInputBuilder(GetLinksInput);

impl GetLinksInputBuilder {
    /// Create a new `GetLinksInputBuilder` from the required fields for a `GetLinksInput`.
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

    /// Construct the result of the builder.
    pub fn build(self) -> GetLinksInput {
        self.0
    }
}
