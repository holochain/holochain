use crate::prelude::*;
use derive_more::Constructor;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct CapToken;
#[derive(Clone, Constructor, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct CapabilityRequest {
    cap_token: CapToken,
    signature: holochain_keystore::Signature,
    agent_pub_key: holo_hash::AgentPubKey,
}
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct DhtOp;
#[derive(Clone, Debug, Default, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct LogRules;
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct Sim2hConfig;

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

#[derive(Default)]
pub struct SourceChainCommitBundle<'env>(std::marker::PhantomData<&'env ()>);
impl<'env> SourceChainCommitBundle<'env> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

/// The value type of the sys-meta database
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {}

/// The value type of the link-meta database
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum LinkMetaVal {}
