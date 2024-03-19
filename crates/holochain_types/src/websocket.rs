//! Common types for WebSocket connections.

use std::collections::HashSet;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

/// Access control for controlling WebSocket connections from browsers.
/// Anywhere other than a browser can set the `Origin` header to any value, so this is only relevant for browser connections.
///
/// See [MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin) for more information.
#[derive(Clone, Debug, PartialEq)]
pub enum AllowedOrigins {
    /// Allow access from any origin.
    Any,
    /// Allow access from a specific origin.
    Origins(HashSet<String>)
}

impl Serialize for AllowedOrigins {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let str: String = self.clone().into();
        serializer.serialize_str(&str)
    }
}

impl<'de> Deserialize<'de> for AllowedOrigins {
    fn deserialize<D>(deserializer: D) -> Result<AllowedOrigins, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.into())
    }
}

impl From<AllowedOrigins> for String {
    fn from(value: AllowedOrigins) -> String {
        match value {
            AllowedOrigins::Any => "*".to_string(),
            AllowedOrigins::Origins(origin) => origin.into_iter().join(","),
        }
    }
}

impl From<String> for AllowedOrigins {
    fn from(value: String) -> AllowedOrigins {
        match value.as_str() {
            "*" => AllowedOrigins::Any,
            _ => {
                AllowedOrigins::Origins(value.split(",").map(|s| s.trim().to_string()).collect())
            },
        }
    }
}

impl std::fmt::Display for AllowedOrigins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str: String = self.clone().into();
        write!(f, "{}", str)
    }
}

impl AllowedOrigins {
    /// Check if the `Origin` header value is allowed.
    pub fn is_allowed(&self, origin: &str) -> bool {
        match self {
            AllowedOrigins::Any => true,
            AllowedOrigins::Origins(allowed) => allowed.contains(origin),
        }
    }
}
