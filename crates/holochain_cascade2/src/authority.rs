use super::error::CascadeResult;
use holo_hash::AgentPubKey;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_types::prelude::*;
use tracing::*;

#[instrument(skip(_state_env))]
pub fn handle_get_entry(
    _state_env: EnvRead,
    _hash: EntryHash,
    _options: holochain_p2p::event::GetOptions,
) -> CascadeResult<GetElementResponse> {
    todo!()
}

#[tracing::instrument(skip(_env))]
pub fn handle_get_element(_env: EnvRead, _hash: HeaderHash) -> CascadeResult<GetElementResponse> {
    todo!()
}

#[instrument(skip(_env))]
pub fn handle_get_agent_activity(
    _env: EnvRead,
    _agent: AgentPubKey,
    _query: ChainQueryFilter,
    _options: holochain_p2p::event::GetActivityOptions,
) -> CascadeResult<AgentActivityResponse> {
    todo!()
}

#[instrument(skip(_env, _options))]
pub fn handle_get_links(
    _env: EnvRead,
    _link_key: WireLinkMetaKey,
    _options: holochain_p2p::event::GetLinksOptions,
) -> CascadeResult<GetLinksResponse> {
    todo!()
}
