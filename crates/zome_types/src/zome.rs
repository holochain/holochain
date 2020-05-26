use holochain_serialized_bytes::prelude::*;

/// ZomeName as a String
#[derive(Clone, Debug, Serialize, Deserialize, Ord, Eq, PartialEq, PartialOrd)]
pub struct ZomeName(pub String);

impl ZomeName {
    pub fn unknown() -> Self {
        "UnknownZomeName".into()
    }
}

impl std::fmt::Display for ZomeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ZomeName {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
