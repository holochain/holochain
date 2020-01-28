
pub type Address = String;
pub type Content = JsonString;
pub type JsonString = String;

pub trait AddressableContent {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentId;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPubKey;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapToken;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityRequest;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DhtTransform;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Dna;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry;

pub struct Lib3hToClient;
pub struct Lib3hToClientResponse;
pub struct Lib3hClientProtocol;
pub struct Lib3hToServer;
pub struct Lib3hToServerResponse;
pub struct Lib3hServerProtocol;

pub struct PersistenceError;
pub type PersistenceResult<T> = Result<T, PersistenceError>;

pub enum ValidationResult {
    Valid,
    Invalid,
    Pending,
}
