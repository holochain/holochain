use mockall::automock;
use sx_types::{entry::Entry, error::SkunkResult, prelude::*};
use std::collections::HashSet;
#[automock]
pub trait NetRequester {
    fn fetch_entry(&self, address: &Address) -> SkunkResult<Option<Entry>>;
    fn fetch_links(&self, base: &Address, tag: String) -> SkunkResult<HashSet<Address>>;
}