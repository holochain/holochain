pub type Address = String;
pub trait AddressableContent {}
#[derive(Clone, PartialEq, Hash, Eq)]
pub struct AgentId;
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AgentPubKey;
pub struct CapToken;
pub struct CapabilityRequest;
pub struct DhtTransform;
pub struct Dna;
pub struct Entry;
pub type JsonString = String;
pub type Content = JsonString;

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
