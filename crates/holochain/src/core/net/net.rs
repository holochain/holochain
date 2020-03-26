use mockall::automock;
use sx_types::{entry::Entry, error::SkunkResult, prelude::*};
#[automock]
pub trait NetRequester {
    fn fetch_entry(&self, address: &Address) -> SkunkResult<Option<Entry>>;
}