//! A `Zome` is a module of app-defined code which can be run by Holochain.
//! A group of Zomes are composed to form a `DnaDef`.
//!
//! Real-world Holochain Zomes are written in Wasm.
//! This module also provides for an "inline" zome definition, which is written
//! using Rust closures, and is useful for quickly defining zomes on-the-fly
//! for tests.

use holochain_serialized_bytes::prelude::*;
use std::borrow::Cow;

/// ZomeName as a String.
#[derive(Clone, Debug, Serialize, Hash, Deserialize, Ord, Eq, PartialEq, PartialOrd)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[repr(transparent)]
pub struct ZomeName(pub Cow<'static, str>);

impl ZomeName {
    /// Create an unknown zome name.
    pub fn unknown() -> Self {
        "UnknownZomeName".into()
    }

    /// Create a zome name from a string.
    pub fn new<S: ToString>(s: S) -> Self {
        ZomeName(s.to_string().into())
    }
}

impl std::fmt::Display for ZomeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ZomeName {
    fn from(s: &str) -> Self {
        Self(s.to_string().into())
    }
}

impl From<String> for ZomeName {
    fn from(s: String) -> Self {
        Self(s.into())
    }
}

/// A single function name.
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, PartialOrd, Ord, Eq, Hash)]
pub struct FunctionName(pub String);

impl FunctionName {
    /// Create a new function name.
    pub fn new<S: ToString>(s: S) -> Self {
        FunctionName(s.to_string())
    }
}

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
