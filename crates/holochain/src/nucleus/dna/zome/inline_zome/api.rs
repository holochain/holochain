//! The Host Function API, as exposed to InlineZomes. This contains the host
//! functions available to WasmZomes, as well as a few extras for convenience.

use holo_hash::AnyDhtHash;
use holo_hash::HeaderHash;
use holochain_types::Entry;
use holochain_zome_types::entry::GetOptions;
use holochain_zome_types::entry_def::EntryDefId;

use super::error::InlineZomeResult;

/// The API which will be passed into each inline zome function, allowing the
/// zome to call host functions
pub struct InlineHostApi {}

impl InlineHostApi {
    /// The `create_entry` host function
    pub fn create_entry<D: Into<EntryDefId>, E: Into<Entry>>(
        &self,
        _entry_def_id: D,
        _entry: E,
    ) -> InlineZomeResult<HeaderHash> {
        todo!()
    }

    /// The `get` host function
    pub fn get<H: Into<AnyDhtHash>>(
        &self,
        hash: H,
        options: GetOptions,
    ) -> InlineZomeResult<HeaderHash> {
        todo!()
    }
}
