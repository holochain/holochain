use kitsune2_api::{DynLocalAgent, SpaceId};
use kitsune2_core::Ed25519LocalAgent;
use kitsune2_test_utils::agent::AgentBuilder;
use std::sync::Arc;

pub fn make_agent(space: &SpaceId) -> String {
    let mut builder = AgentBuilder::default();
    let local_agent: DynLocalAgent = Arc::new(Ed25519LocalAgent::default());
    builder.space = Some(space.clone());
    let info = builder.build(local_agent);
    info.encode().unwrap()
}
