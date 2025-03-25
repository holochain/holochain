use super::Conductor;
use crate::conductor::api::error::{ConductorApiError, ConductorApiResult};
use chrono::{DateTime, Utc};
use hdk::prelude::CellId;
use holochain_conductor_api::{AgentInfoDump, P2pAgentsDump};
use kitsune2_api::AgentInfoSigned;
use std::sync::Arc;

pub async fn peer_store_dump(
    conductor: &Conductor,
    cell_id: &CellId,
) -> ConductorApiResult<P2pAgentsDump> {
    let dna_hash = cell_id.dna_hash().clone();
    let peer_store = conductor
        .holochain_p2p
        .peer_store(dna_hash.clone())
        .await
        .map_err(|err| ConductorApiError::CellError(err.into()))?;
    let all_peers = peer_store.get_all().await?;
    let agent_id = cell_id.agent_pubkey().to_k2_agent();
    let this_agent_info = peer_store.get(agent_id.clone()).await?;
    let agent_pub_key = cell_id.agent_pubkey().clone();
    let space_id = dna_hash.to_k2_space();
    Ok(P2pAgentsDump {
        peers: all_peers.into_iter().map(agent_info_dump).collect(),
        this_agent_info: this_agent_info.map(agent_info_dump),
        this_agent: Some((agent_pub_key, agent_id)),
        this_dna: Some((dna_hash, space_id)),
    })
}

fn agent_info_dump(peer: Arc<AgentInfoSigned>) -> AgentInfoDump {
    let created_at = DateTime::from_timestamp_micros(peer.created_at.as_micros())
        .expect("Agents signed 262,000 years from now are irrelevant");
    let exp = DateTime::from_timestamp_micros(peer.expires_at.as_micros())
        .expect("Agents expiring 262,000 years from now are irrelevant");
    let now = Utc::now();
    let dump = format!(
        r#"created at {}
expires at {} in {} min
url: {:?}"#,
        created_at,
        exp,
        (exp - now).num_minutes(),
        peer.url
    );
    AgentInfoDump {
        dump,
        kitsune_agent: Arc::new(peer.agent.clone()),
        kitsune_space: Arc::new(peer.space.clone()),
    }
}
