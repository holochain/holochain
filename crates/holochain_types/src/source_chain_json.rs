//! Types for serializing a source chain so that it can be inspected by external tools

use holo_hash::HeaderHash;
use holochain_zome_types::{Entry, Header, Signature};

use serde::{Deserialize, Serialize};

#[allow(missing_docs)]
// TODO fix this.  We shouldn't really have nil values but this would
// show if the database is corrupted and doesn't have an element
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct SourceChainJsonDump {
    pub elements: Vec<SourceChainJsonElement>,
    pub published_ops_count: usize,
}

#[allow(missing_docs)]
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct SourceChainJsonElement {
    pub signature: Signature,
    pub header_address: HeaderHash,
    pub header: Header,
    pub entry: Option<Entry>,
}
