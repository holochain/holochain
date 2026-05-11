//! Cell configuration overrides module.
//!
//! This module defines structures and functions for overriding
//! Cell configuration settings in Holochain applications.

/// Overrides for Cell configuration settings.
///
/// This struct holds optional override values for Cell configurations
/// such as bootstrap URLs, signal server URLs, and authentication material.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CellConfigOverrides {
    /// URL of the bootstrap server to use for all Cells created
    /// for an app. If not overridden, the bootstrap server
    /// specified in the conductor config file will be used.
    pub bootstrap_url: Option<String>,
    /// URL of the relay server to use for all Cells created
    /// for an app. If not overridden, the relay server
    /// specified in the conductor config file will be used.
    pub relay_url: Option<String>,
    /// Base64-encoded authentication material for bootstrap/signal services.
    /// If not overridden, the auth material specified in the conductor
    /// config file will be used.
    pub base64_auth_material: Option<String>,
}

impl CellConfigOverrides {
    /// Check if any override is set.
    ///
    /// Returns `true` if at least one override field is [`Some`], otherwise returns `false`.
    pub fn is_overriding(&self) -> bool {
        self.bootstrap_url.is_some()
            || self.base64_auth_material.is_some()
            || self.relay_url.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_tell_whether_is_overriding() {
        let overrides = CellConfigOverrides {
            bootstrap_url: None,
            base64_auth_material: None,
            relay_url: None,
        };
        assert!(!overrides.is_overriding());

        let overrides = CellConfigOverrides {
            bootstrap_url: Some("http://localhost:1234".to_string()),
            base64_auth_material: None,
            relay_url: None,
        };
        assert!(overrides.is_overriding());

        let overrides = CellConfigOverrides {
            bootstrap_url: None,
            base64_auth_material: None,
            relay_url: None,
        };
        assert!(overrides.is_overriding());

        let overrides = CellConfigOverrides {
            bootstrap_url: Some("http://localhost:1234".to_string()),
            base64_auth_material: None,
            relay_url: None,
        };
        assert!(overrides.is_overriding());

        let overrides = CellConfigOverrides {
            bootstrap_url: None,
            base64_auth_material: Some("dGVzdA==".to_string()),
            relay_url: None,
        };
        assert!(overrides.is_overriding());

        let overrides = CellConfigOverrides {
            bootstrap_url: None,
            base64_auth_material: None,
            relay_url: Some("http://relay.example.com/relay".to_string()),
        };
        assert!(overrides.is_overriding());
    }
}
