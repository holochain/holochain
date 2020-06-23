use crate::prelude::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct LogRules;
#[derive(Clone, Debug, Serialize, Deserialize, SerializedBytes, PartialEq, Eq)]
pub struct Sim2hConfig;

pub enum ValidationResult {
    Valid,
    Invalid,
    Pending,
}
