use holochain_serialized_bytes::prelude::*;

/// ZomeName as a String.
#[derive(Clone, Debug, Serialize, Hash, Deserialize, Ord, Eq, PartialEq, PartialOrd)]
#[repr(transparent)]
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
        Self::from(s.to_string())
    }
}

impl From<String> for ZomeName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A single function name.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, PartialOrd, Ord, Eq, Hash)]
pub struct FunctionName(pub String);

impl std::fmt::Display for FunctionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<FunctionName> for String {
    fn from(function_name: FunctionName) -> Self {
        function_name.0
    }
}

impl From<String> for FunctionName {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for FunctionName {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

impl AsRef<str> for FunctionName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}
