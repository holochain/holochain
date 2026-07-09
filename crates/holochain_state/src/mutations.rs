mod error;

use crate::scratch::Scratch;
use holochain_types::prelude::*;

pub use self::error::*;

pub fn insert_record_scratch(
    scratch: &mut Scratch,
    record: Record,
    chain_top_ordering: ChainTopOrdering,
) {
    let (action, entry) = record.into_inner();
    scratch.add_action(action, chain_top_ordering);
    if let Some(entry) = entry.into_option() {
        scratch.add_entry(EntryHashed::from_content_sync(entry), chain_top_ordering);
    }
}
