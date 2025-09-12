use kitsune2_api::{DynLocalAgent, SpaceId};
use kitsune2_core::Ed25519LocalAgent;
use kitsune2_test_utils::agent::AgentBuilder;
use std::sync::Arc;

pub fn make_agent(space: &SpaceId) -> String {
    AgentBuilder {
        space_id: Some(space.clone()),
        ..Default::default()
    }
    .build(Arc::new(Ed25519LocalAgent::default()) as DynLocalAgent)
    .encode()
    .unwrap()
}
