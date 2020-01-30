use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Hash, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum DnaError {
    ZomeNotFound(String),
    TraitNotFound(String),
    ZomeFunctionNotFound(String),
}

impl Error for DnaError {}

impl fmt::Display for DnaError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match self {
            DnaError::ZomeNotFound(err_msg) => err_msg,
            DnaError::TraitNotFound(err_msg) => err_msg,
            DnaError::ZomeFunctionNotFound(err_msg) => err_msg,
        };
        write!(f, "{}", msg)
    }
}
