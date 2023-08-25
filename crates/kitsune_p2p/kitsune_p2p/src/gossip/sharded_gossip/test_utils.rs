use std::sync::Arc;

use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash},
};

use crate::gossip::MetaOpKey;

use super::*;

/// Create an agent bloom for testing.
pub fn create_agent_bloom<'a>(
    agents: impl Iterator<Item = &'a AgentInfoSigned>,
    filter: Option<&AgentInfoSigned>,
) -> Option<BloomFilter> {
    let agents: Vec<_> = match filter {
        Some(filter) => agents
            .filter(|a| filter.storage_arc.contains(a.agent.get_loc()))
            .collect(),
        None => agents.collect(),
    };
    let mut bloom = bloomfilter::Bloom::new_for_fp_rate(agents.len(), 0.01);
    let empty = agents.is_empty();
    for info in agents {
        let signed_at_ms = info.signed_at_ms;
        // The key is the agent hash + the signed at.
        let key = MetaOpKey::Agent(info.0.agent.clone(), signed_at_ms);
        bloom.set(&key);
    }
    if empty {
        None
    } else {
        Some(BloomFilter::from(bloom))
    }
}

/// Create an ops bloom for testing.
pub fn create_op_bloom(ops: Vec<Arc<KitsuneOpHash>>) -> BloomFilter {
    let len = ops.len();
    let bloom = ops.into_iter().fold(
        bloomfilter::Bloom::new_for_fp_rate(len, 0.01),
        |mut bloom, op| {
            let key = MetaOpKey::Op(op);
            bloom.set(&key);
            bloom
        },
    );

    bloom.into()
}

/// Check an ops bloom for testing.
pub fn check_ops_bloom<'a>(
    ops: impl Iterator<Item = (kitsune_p2p_timestamp::Timestamp, &'a Arc<KitsuneOpHash>)>,
    bloom: TimedBloomFilter,
) -> Vec<&'a Arc<KitsuneOpHash>> {
    match bloom {
        TimedBloomFilter::NoOverlap => vec![],
        TimedBloomFilter::MissingAllHashes { window } => ops
            .filter(|(t, _)| window.contains(t))
            .map(|(_, h)| h)
            .collect(),
        TimedBloomFilter::HaveHashes { bloom, window } => ops
            .filter(|(t, _)| window.contains(t))
            .map(|(_, h)| h)
            .filter(|op| !bloom.check(&MetaOpKey::Op((**op).clone())))
            .collect(),
    }
}

/// Check an ops bloom for testing.
pub fn check_agent_boom<'a>(
    agents: impl Iterator<Item = (&'a Arc<KitsuneAgent>, &'a AgentInfoSigned)>,
    bloom: &BloomFilter,
) -> Vec<&'a Arc<KitsuneAgent>> {
    agents
        .filter(|(agent, info)| {
            !bloom.check(&MetaOpKey::Agent((*agent).clone(), info.signed_at_ms))
        })
        .map(|(a, _)| a)
        .collect()
}
