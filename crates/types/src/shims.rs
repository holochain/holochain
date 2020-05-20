use crate::prelude::*;

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

/// The value type of the sys-meta database
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum SysMetaVal {}

/// The value type of the link-meta database
#[derive(Debug, Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum LinkMetaVal {}
