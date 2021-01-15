use hdk3::prelude::*;

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AnchorInput(pub String, pub String);

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct AgentActivitySearch {
    pub agent: AgentPubKey,
    pub query: QueryFilter,
    pub request: ActivityRequest,
}
