use crate::prelude::*;
use derive_more::Constructor;
use fixt::prelude::*;

#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct CapToken;

fixturator!(CapToken, CapToken, CapToken, CapToken);

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
