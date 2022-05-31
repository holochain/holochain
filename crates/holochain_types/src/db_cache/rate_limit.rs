//! Cached DB state for rate limiting

use std::{collections::HashMap, sync::Arc};

use holo_hash::AgentPubKey;
use holochain_zome_types::prelude::*;

/// The cached state of rate buckets for the agents in this DHT space
pub struct RateLimitDbCache {
    limits: Vec<Arc<RateLimit>>,
    buckets: HashMap<AgentPubKey, HashMap<u8, BucketState>>,
}

impl RateLimitDbCache {
    /// Change the state of the rate limit bucket for this Header.
    /// An error implies that this header is invalid.
    pub fn process_header(&mut self, header: &Header) -> RateBucketResult<()> {
        let author = header.author().clone();
        let timestamp = header.timestamp();
        let LinkWeight {
            rate_bucket: index,
            rate_weight: weight,
        } = header.rate_data();

        let params: Arc<RateLimit> = self
            .limits
            .get(index as usize)
            .ok_or_else(|| RateBucketError::BucketIdMissing(index))?
            .clone();

        let bucket = self
            .buckets
            .entry(author)
            .or_insert(HashMap::new())
            .entry(index)
            .or_insert(BucketState::new(params, index));

        bucket.change(weight, timestamp)?;
        Ok(())
    }
}
