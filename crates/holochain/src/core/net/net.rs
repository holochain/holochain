#![allow(clippy::ptr_arg)]
use mockall::automock;
use std::collections::HashSet;
use sx_types::{entry::Entry, error::SkunkResult, prelude::*};
#[automock]
pub trait NetRequester {
    fn fetch_entry(&self, address: &Address) -> SkunkResult<Option<Entry>>;
    fn fetch_links(&self, base: &Address, tag: String) -> SkunkResult<HashSet<Address>>;
}
