use super::*;
use holochain_serialized_bytes::SerializedBytes;
use std::collections::BTreeMap;

/// Map of zome functions to the payloads to curry into to them
// @todo Ability to forcibly curry payloads into functions that are called with a claim.
#[derive(Default, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurryPayloads(pub BTreeMap<GrantedFunction, SerializedBytes>);
