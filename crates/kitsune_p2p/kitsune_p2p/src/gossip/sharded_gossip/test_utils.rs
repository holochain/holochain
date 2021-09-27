use std::sync::Arc;

use kitsune_p2p_types::{
    agent_info::AgentInfoSigned, bin_types::KitsuneBinType, tx2::tx2_utils::PoolBuf,
};

use crate::gossip::simple_bloom::{encode_bloom_filter, MetaOpKey};

pub fn create_agent_bloom<'a>(
    agents: impl Iterator<Item = &'a AgentInfoSigned>,
    filter: Option<&AgentInfoSigned>,
) -> PoolBuf {
    let agents: Vec<_> = match filter {
        Some(filter) => agents
            .filter(|a| filter.storage_arc.contains(a.agent.get_loc()))
            .collect(),
        None => agents.collect(),
    };
    let mut bloom = bloomfilter::Bloom::new_for_fp_rate(agents.len(), 0.0001);
    for info in agents {
        let signed_at_ms = info.signed_at_ms;
        // The key is the agent hash + the signed at.
        let key = Arc::new(MetaOpKey::Agent(info.0.agent.clone(), signed_at_ms));
        bloom.set(&key);
    }
    encode_bloom_filter(&bloom)
}
