//! Cached DB state for rate limiting

use std::collections::HashMap;

use holo_hash::AgentPubKey;
use holochain_zome_types::prelude::*;

pub struct RateLimitDbCache {
    limits: RateLimits,
    buckets: HashMap<AgentPubKey, HashMap<RateBucketId, BucketState>>,
}


impl RateLimitDbCache {
    pub fn process_header(&mut self, header: &Header) {
        let author = header.author().clone();
        let LinkWeight {
            rate_bucket: bucket,
            rate_weight: weight,
        } = header.rate_data();

        self.buckets.entry(agent).and_modify(|m|
            m.entry(bucket).and_modify(|b| ))
    }

    /// Get the mutable bucket capacity given the two keys which form a path
    /// to that inner value. Creates empty/initial values along the way if a
    /// hash map or value is missing.
    fn bucket_mut(&mut self, agent: &AgentPubKey, bucket: RateBucketId) -> Result<&mut BucketState, String> {
        let inner = if let Some(inner) = self.buckets.get_mut(agent) {
            inner
        } else {
            self.buckets.insert([(bucket)])
        }
    }
}
