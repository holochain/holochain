use crate::{prelude::*, signature::Provenance};
use derive_more::Constructor;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct CapToken;
#[derive(Clone, Constructor, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct CapabilityRequest {
    cap_token: CapToken,
    provenance: Provenance,
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
pub struct Keystore;
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
