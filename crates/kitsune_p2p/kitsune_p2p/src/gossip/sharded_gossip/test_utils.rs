use std::sync::Arc;

use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneBinType, KitsuneOpHash},
    tx2::tx2_utils::PoolBuf,
};

use crate::gossip::simple_bloom::{decode_bloom_filter, encode_bloom_filter, MetaOpKey};

use super::EncodedTimedBloomFilter;

/// Create an agent bloom for testing.
pub fn create_agent_bloom<'a>(
    agents: impl Iterator<Item = &'a AgentInfoSigned>,
    filter: Option<&AgentInfoSigned>,
) -> Option<PoolBuf> {
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
        Some(encode_bloom_filter(&bloom))
    }
}

/// Create an ops bloom for testing.
pub fn create_ops_bloom(ops: Vec<Arc<KitsuneOpHash>>) -> PoolBuf {
    let len = ops.len();
    let bloom = ops.into_iter().fold(
        bloomfilter::Bloom::new_for_fp_rate(len, 0.01),
        |mut bloom, op| {
            let key = MetaOpKey::Op(op);
            bloom.set(&key);
            bloom
        },
    );

    encode_bloom_filter(&bloom)
}

/// Check an ops bloom for testing.
pub fn check_ops_boom<'a>(
    ops: impl Iterator<Item = (kitsune_p2p_timestamp::Timestamp, &'a Arc<KitsuneOpHash>)>,
    bloom: EncodedTimedBloomFilter,
) -> Vec<&'a Arc<KitsuneOpHash>> {
    match bloom {
        EncodedTimedBloomFilter::NoOverlap => vec![],
        EncodedTimedBloomFilter::MissingAllHashes { time_window } => ops
            .filter(|(t, _)| time_window.contains(t))
            .map(|(_, h)| h)
            .collect(),
        EncodedTimedBloomFilter::HaveHashes {
            filter,
            time_window,
        } => {
            let filter = decode_bloom_filter(&filter);
            ops.filter(|(t, _)| time_window.contains(t))
                .map(|(_, h)| h)
                .filter(|op| !filter.check(&MetaOpKey::Op((**op).clone())))
                .collect()
        }
    }
}

/// Check an ops bloom for testing.
pub fn check_agent_boom<'a>(
    agents: impl Iterator<Item = (&'a Arc<KitsuneAgent>, &'a AgentInfoSigned)>,
    bloom: &[u8],
) -> Vec<&'a Arc<KitsuneAgent>> {
    let filter = decode_bloom_filter(bloom);
    agents
        .filter(|(agent, info)| {
            !filter.check(&MetaOpKey::Agent((*agent).clone(), info.signed_at_ms))
        })
        .map(|(a, _)| a)
        .collect()
}
