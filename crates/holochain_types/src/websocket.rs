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

#[cfg(test)]
mod tests {
    use super::AllowedOrigins;

    #[test]
    fn any_origin_to_and_from_string() {
        let allowed_origins = AllowedOrigins::Any;
        let str: String = allowed_origins.clone().into();
        let allowed_origins_2 = str.clone().into();

        assert_eq!("*".to_string(), str);
        assert_eq!(allowed_origins, allowed_origins_2);
    }

    #[test]
    fn single_origin_to_and_from_string() {
        let allowed_origins = AllowedOrigins::Origins(["http://example.com".to_string()].iter().cloned().collect());
        let str: String = allowed_origins.clone().into();
        let allowed_origins_2 = str.clone().into();

        assert_eq!("http://example.com".to_string(), str);
        assert_eq!(allowed_origins, allowed_origins_2);
    }

    #[test]
    fn multiple_origins_to_and_from_string() {
        let allowed_origins = AllowedOrigins::Origins(["http://example1.com".to_string(), "http://example2.com".to_string()].iter().cloned().collect());
        let str: String = allowed_origins.clone().into();
        let allowed_origins_2 = str.into();

        assert_eq!(allowed_origins, allowed_origins_2);
    }

    #[test]
    fn any_origin_is_allowed() {
        let allowed_origins = AllowedOrigins::Any;
        assert!(allowed_origins.is_allowed("http://example.com"));
    }

    #[test]
    fn specific_origin_is_allowed() {
        let allowed_origins = AllowedOrigins::Origins(["http://example.com".to_string()].iter().cloned().collect());
        assert!(allowed_origins.is_allowed("http://example.com"));
    }

    #[test]
    fn other_origin_is_not_allowed() {
        let allowed_origins = AllowedOrigins::Origins(["http://example.com".to_string()].iter().cloned().collect());
        assert!(!allowed_origins.is_allowed("http://example2.com"));
    }

    #[test]
    fn multiple_origins_ignores_whitespace() {
        let str = " http://example1.com , http://example2.com,\thttp://example3.com\n";

        let origins = AllowedOrigins::from(str.to_string());
        assert!(origins.is_allowed("http://example1.com"));
        assert!(origins.is_allowed("http://example2.com"));
        assert!(origins.is_allowed("http://example3.com"));
    }

    #[test]
    fn serialize_deserialize() {
        let allowed_origins = AllowedOrigins::Origins(["http://example1.com".to_string(), "http://example2.com".to_string()].iter().cloned().collect());
        let serialized = serde_json::to_string(&allowed_origins).unwrap();
        let deserialized: AllowedOrigins = serde_json::from_str(&serialized).unwrap();
        assert_eq!(allowed_origins, deserialized);
    }
}
