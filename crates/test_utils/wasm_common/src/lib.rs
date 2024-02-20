use hdk::prelude::*;

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
pub struct AnchorInput(pub String, pub String);

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
pub struct ManyAnchorInput(pub Vec<AnchorInput>);

#[derive(Clone, serde::Serialize, serde::Deserialize, SerializedBytes, Debug)]
pub struct AgentActivitySearch {
    pub agent: AgentPubKey,
    pub query: QueryFilter,
    pub request: ActivityRequest,
}

#[derive(Eq, PartialEq, Clone)]
#[dna_properties]
pub struct MyValidDnaProperties {
    pub authority_agent: Vec<u8>,
    pub max_count: u32,
    pub contract_address: String,
}
