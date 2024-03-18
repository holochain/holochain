//! Common types for WebSocket connections.

use serde::{Deserialize, Serialize};

/// Access control for controlling WebSocket connections from browsers.
/// Anywhere other than a browser can set the `Origin` header to any value, so this is only relevant for browser connections.
///
/// See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin) for more information.
#[derive(Clone, Debug, PartialEq)]
pub enum AllowedOrigin {
    /// Allow access from any origin.
    Any,
    /// Allow access from a specific origin.
    Origin(String)
}

impl Serialize for AllowedOrigin {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let str: String = self.clone().into();
        serializer.serialize_str(&str)
    }
}

impl<'de> Deserialize<'de> for AllowedOrigin {
    fn deserialize<D>(deserializer: D) -> Result<AllowedOrigin, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.into())
    }
}

impl From<AllowedOrigin> for String {
    fn from(value: AllowedOrigin) -> String {
        match value {
            AllowedOrigin::Any => "*".to_string(),
            AllowedOrigin::Origin(origin) => origin.to_string(),
        }
    }
}

impl From<String> for AllowedOrigin {
    fn from(value: String) -> AllowedOrigin {
        match value.as_str() {
            "*" => AllowedOrigin::Any,
            _ => AllowedOrigin::Origin(value),
        }
    }
}

impl AllowedOrigin {
    /// Check if the `Origin` header value is allowed.
    pub fn is_allowed(&self, origin: &str) -> bool {
        match self {
            AllowedOrigin::Any => true,
            AllowedOrigin::Origin(allowed) => origin == *allowed,
        }
    }
}