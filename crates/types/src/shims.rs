use holochain_json_api::{error::JsonError, json::JsonString};

#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, PartialEq, Eq)]
pub struct AgentPubKey;
#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, PartialEq, Eq)]
pub struct CapToken;
#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, PartialEq, Eq)]
pub struct CapabilityRequest;
#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, PartialEq, Eq)]
pub struct DhtTransform;
#[derive(Clone, Debug, Default, Serialize, Deserialize, DefaultJson, PartialEq, Eq)]
pub struct LogRules;
#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, PartialEq, Eq)]
pub struct Sim2hConfig;

#[derive(Clone, Debug, Serialize, Deserialize, DefaultJson, PartialEq, Eq)]
pub struct Entry;

pub struct Lib3hToClient;
pub struct Lib3hToClientResponse;
pub struct Lib3hClientProtocol;
pub struct Lib3hToServer;
pub struct Lib3hToServerResponse;
pub struct Lib3hServerProtocol;


pub enum ValidationResult {
    Valid,
    Invalid,
    Pending,
}
