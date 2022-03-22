use super::*;
use holochain_serialized_bytes::SerializedBytes;
use std::collections::BTreeMap;

#[derive(Default, PartialEq, Eq, Debug, Clone, serde::Serialize, serde::Deserialize)]
/// @todo Ability to forcibly curry payloads into functions that are called with a claim.
pub struct CurryPayloads(pub BTreeMap<GrantedFunction, SerializedBytes>);
