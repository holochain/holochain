use crate::address::HeaderAddress;
use crate::nucleus::ZomeName;
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

type FailString = String;

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct ValidationPackage;

pub enum ValidationStatus {
    Valid,
    Invalid,
    Pending,
    Abandoned,
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidationCallbackResult {
    Valid,
    Invalid(FailString),
    /// subconscious needs to map this to either pending or abandoned based on context that the
    /// wasm can't possibly have
    UnresolvedDependencies(Vec<EntryHash>),
}

pub enum InitDnaResult {
    Pass,
    // zome name, error
    Fail(ZomeName, FailString),
    UnresolvedDependencies(ZomeName, Vec<EntryHash>),
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum InitCallbackResult {
    Pass,
    Fail(FailString),
    UnresolvedDependencies(Vec<EntryHash>),
}

pub enum AgentMigrateDnaDirection {
    Open,
    Close,
}

pub enum AgentMigrateDnaResult {
    Pass,
    Fail(ZomeName, FailString),
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum AgentMigrateCallbackResult {
    Pass,
    Fail(FailString),
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum ValidationPackageCallbackResult {
    Success(ValidationPackage),
    Fail(FailString),
}

#[derive(PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum PostCommitCallbackResult {
    Success(HeaderAddress),
    Fail(HeaderAddress, FailString),
}

#[derive(Default)]
pub struct SourceChainCommitBundle<'env>(std::marker::PhantomData<&'env ()>);
impl<'env> SourceChainCommitBundle<'env> {
    pub fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}
