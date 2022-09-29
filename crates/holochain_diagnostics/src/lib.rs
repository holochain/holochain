pub use holochain;

use holochain::conductor::config::ConductorConfig;
pub use rand::rngs::StdRng;
pub use rand::Rng;
use rand::*;

/// A RNG suitable for testing, if no seed is passed, uses standard random seed.
pub fn seeded_rng(seed: Option<u64>) -> StdRng {
    let seed = seed.unwrap_or_else(|| thread_rng().gen());
    tracing::info!("RNG seed: {}", seed);
    StdRng::seed_from_u64(seed)
}

pub fn standard_config() -> ConductorConfig {
    holochain::sweettest::standard_config()
}

pub fn config_no_publish() -> ConductorConfig {
    let mut config = standard_config();
    config.network.as_mut().map(|c| {
        *c = c.clone().tune(|mut tp| {
            tp.disable_publish = true;
            tp
        });
    });
    config
}

pub fn config_historical_only() -> ConductorConfig {
    let mut config = standard_config();
    config.network.as_mut().map(|c| {
        *c = c.clone().tune(|mut tp| {
            tp.disable_publish = true;
            tp.disable_recent_gossip = true;
            tp
        });
    });
    config
}

pub fn config_historical_and_agent_gossip_only() -> ConductorConfig {
    let mut config = standard_config();
    config.network.as_mut().map(|c| {
        *c = c.clone().tune(|mut tp| {
            tp.disable_publish = true;
            // keep recent gossip for agent gossip, but gossip no ops.
            tp.danger_gossip_recent_threshold_secs = 0;
            tp
        });
    });
    config
}

pub fn config_recent_only() -> ConductorConfig {
    let mut config = standard_config();
    config.network.as_mut().map(|c| {
        *c = c.clone().tune(|mut tp| {
            tp.disable_publish = true;
            tp.disable_historical_gossip = true;
            tp
        });
    });
    config
}
