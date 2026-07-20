use holochain_integrity_types::prelude::GrantedFunction;
use holochain_serialized_bytes::SerializedBytes;
use std::collections::BTreeMap;

/// Map of zome functions to the payloads to curry into to them
#[derive(Default, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurryPayloads(pub BTreeMap<GrantedFunction, SerializedBytes>);
