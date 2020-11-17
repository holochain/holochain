use crate::core::ribosome::error::RibosomeResult;
use holo_hash::{AnyDhtHash, HeaderHash};
use holochain_types::Entry;
use holochain_zome_types::{entry::GetOptions, entry_def::EntryDefId};

pub struct InlineHostApi {}

impl InlineHostApi {
    pub fn create_entry<D: Into<EntryDefId>, E: Into<Entry>>(
        &self,
        _entry_def_id: D,
        _entry: E,
    ) -> RibosomeResult<HeaderHash> {
        todo!()
    }

    pub fn get<H: Into<AnyDhtHash>>(
        &self,
        hash: H,
        options: GetOptions,
    ) -> RibosomeResult<HeaderHash> {
        todo!()
    }
}
