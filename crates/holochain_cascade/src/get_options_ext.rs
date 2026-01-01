//! Extension trait to convert GetOptions to NetworkRequestOptions.

use holochain_p2p::actor::NetworkRequestOptions;
use holochain_zome_types::prelude::GetOptions;

/// Extension trait for converting GetOptions to NetworkRequestOptions.
pub trait GetOptionsExt {
    /// Convert GetOptions to NetworkRequestOptions for use in network calls.
    /// This takes the configured options or falls back to defaults.
    fn to_network_options(&self) -> NetworkRequestOptions;
}

impl GetOptionsExt for GetOptions {
    fn to_network_options(&self) -> NetworkRequestOptions {
        NetworkRequestOptions {
            remote_agent_count: self.remote_agent_count().unwrap_or(3),
            timeout_ms: self.timeout_ms(),
            as_race: self.as_race().unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_network_options() {
        let options = GetOptions::network()
            .with_remote_agent_count(7)
            .with_timeout_ms(5000)
            .with_aggregation();

        let network_options = options.to_network_options();
        assert_eq!(network_options.remote_agent_count, 7);
        assert_eq!(network_options.timeout_ms, Some(5000));
        assert_eq!(network_options.as_race, false);

        // Test defaults
        let options = GetOptions::network();
        let network_options = options.to_network_options();
        assert_eq!(network_options.remote_agent_count, 3);
        assert_eq!(network_options.timeout_ms, None);
        assert_eq!(network_options.as_race, true);
    }
}
