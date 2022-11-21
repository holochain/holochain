use std::sync::Arc;

use kitsune_p2p_types::bin_types::*;

/// Determine what should be fetched.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FetchKey {
    /// Fetch via region.
    // TODO - add region info
    Region,

    /// Fetch via op hash.
    Op {
        /// The hash of the op to fetch.
        op_hash: Arc<KitsuneBasis>,

        /// If specified, the author of the op.
        /// NOTE: author is addative-only. That is, an op without an author
        /// is the same as one *with* an author, but should be updated to
        /// include the author. It is UB to have two FetchKeys with the
        /// same op_hash, but different authors.
        author: Option<Arc<KitsuneAgent>>,
    },
}

/// A fetch "unit" that can be de-duplicated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct FetchRequest {
    ///
    key: FetchKey,

    /// Optional arguments related to fetching the data.
    _options: Option<FetchOptions>,
}

/// Options which affect how the fetch is performed
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct FetchOptions {
    __: (),
}
