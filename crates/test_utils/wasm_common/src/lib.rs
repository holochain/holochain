use hdk::prelude::*;

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
pub struct AnchorInput(pub String, pub String);

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
pub struct AgentActivitySearch {
    pub agent: AgentPubKey,
    pub query: QueryFilter,
    pub request: ActivityRequest,
}
