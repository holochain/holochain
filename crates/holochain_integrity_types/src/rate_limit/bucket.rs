use std::sync::Arc;

use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

use crate::{RateBucketCapacity, RateBucketError, RateBucketResult, RateUnits};

/// Defines parameters for a "bucket algorithm" for throttling agent activity.
/// This struct defines such a "bucket".
///
/// Each bucket has a certain fixed capacity, and starts out empty (level=0).
/// Each Action has a bucket and weight assigned to it, and when that Action is
/// authored, it adds an amount of units equal to is weight to its assigned bucket.
/// If a Action causes a bucket's level to exceed its `capacity`, that Action
/// is invalid.
///
/// Every `drain_interval_ms`, the bucket's level is reduced by `drain_amount`.
/// In other words, the bucket is "draining" at a rate of
/// `drain_amount / drain_interval_ms`.
#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize, SerializedBytes)]
pub struct RateLimit {
    /// The total capacity of the bucket, i.e. when this level is reached, the
    /// rate limit is exceeded
    pub capacity: RateBucketCapacity,
    /// How many units "leak" from the bucket per time interval
    pub drain_amount: RateBucketCapacity,
    /// How long is the time interval used to measure the "leak" rate?
    pub drain_interval_ms: u32,
}

impl RateLimit {
    /// A placeholder "default" rate limit imposed on every DNA, to be replaced
    /// with the real deal once we have the mechanism in place to track the
    /// different rate limits across different zomes
    pub fn placeholder() -> Self {
        Self {
            capacity: 1024,
            drain_amount: 1,
            drain_interval_ms: 1000,
        }
    }
}

/// Calculate the next level of a bucket from the previous state.
/// See [`RateLimit`] for details on the bucket algorithm.
///
/// Bucket overflow or nonmonotonic timestamp causes an error.
pub fn next_bucket_level(
    params: &RateLimit,
    prev: Option<(RateBucketCapacity, Timestamp)>,
    weight: RateUnits,
    timestamp: Timestamp,
) -> RateBucketResult<RateBucketCapacity> {
    let (level, drain_amount) = if let Some((prev_level, last_access)) = prev {
        if timestamp <= last_access {
            Err(RateBucketError::NonMonotonicTimestamp(
                last_access,
                timestamp,
            ))?
        }
        let interval = (timestamp - last_access)?
            .num_milliseconds()
            .min(525600 * 60 * 1000) as u32; // 1 year
        let drain = params.drain_amount.saturating_mul(interval) / params.drain_interval_ms;
        (prev_level, drain)
    } else {
        (0, params.capacity)
    };
    let level = level.saturating_sub(drain_amount) + weight as RateBucketCapacity;
    if level > params.capacity {
        Err(RateBucketError::BucketOverflow)
    } else {
        Ok(level)
    }
}

/// Tracks the current level of a bucket, and calculates the next level given
/// an incoming action.
///
/// Mainly useful for testing, as a way to observe the effects of multiple
/// bucket state changes.
#[derive(Debug, PartialEq, Eq)]
pub struct BucketState {
    params: Arc<RateLimit>,
    level: RateBucketCapacity,
    last_access: Option<Timestamp>,
}

impl BucketState {
    /// Constructor
    pub fn new(params: Arc<RateLimit>) -> Self {
        Self {
            params,
            level: 0,
            last_access: None,
        }
    }

    /// Process an item with the given weight at the given time.
    /// Bucket overflow or nonmonotonic timestamp causes an error.
    pub fn change(&mut self, weight: RateUnits, timestamp: Timestamp) -> RateBucketResult<()> {
        self.level = next_bucket_level(
            &self.params,
            self.last_access.map(|a| (self.level, a)),
            weight,
            timestamp,
        )?;
        self.last_access = Some(timestamp);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kitsune_p2p_timestamp::Timestamp;

    #[test]
    fn bucket_drains_completely() {
        let p = Arc::new(RateLimit {
            capacity: 100,
            drain_amount: 10,
            drain_interval_ms: 1,
        });
        let mil = 1000;
        let mut b = BucketState::new(p);
        b.change(100, Timestamp::from_micros(mil * 0)).unwrap();
        assert_eq!(b.level, 100);

        b.change(1, Timestamp::from_micros(mil * 10)).unwrap();
        assert_eq!(b.level, 1);
    }

    #[test]
    fn bucket_monotonicity() {
        let p = Arc::new(RateLimit {
            capacity: 1000,
            drain_amount: 100,
            drain_interval_ms: 1000,
        });
        let mil = 1000;
        let mut b = BucketState::new(p);
        b.change(50, Timestamp::from_micros(mil * 1000)).unwrap();
        assert_eq!(
            dbg!(b.change(1, Timestamp::from_micros(mil * 999))),
            Err(RateBucketError::NonMonotonicTimestamp(
                Timestamp::from_micros(mil * 1000),
                Timestamp::from_micros(mil * 999)
            ))
        );
    }

    #[test]
    fn bucket_behavior() {
        let p = Arc::new(RateLimit {
            capacity: 1000,
            drain_amount: 100,
            drain_interval_ms: 1000,
        });
        let mil = 1000;
        let mut b = BucketState::new(p);

        b.change(200, Timestamp::from_micros(mil * 0)).unwrap();
        assert_eq!(b.level, 200);
        b.change(200, Timestamp::from_micros(mil * 100)).unwrap();
        assert_eq!(b.level, 400 - 10);
        b.change(200, Timestamp::from_micros(mil * 200)).unwrap();
        assert_eq!(b.level, 600 - 20);
        b.change(200, Timestamp::from_micros(mil * 300)).unwrap();
        assert_eq!(b.level, 800 - 30);
        b.change(200, Timestamp::from_micros(mil * 400)).unwrap();
        assert_eq!(b.level, 1000 - 40);

        b.change(50, Timestamp::from_micros(mil * 500)).unwrap();
        assert_eq!(b.level, 1000);
        // bucket is exactly full now.

        assert_eq!(
            b.change(100, Timestamp::from_micros(mil * 1000)),
            Err(RateBucketError::BucketOverflow)
        );
    }

    // fn make_actions<H: Iterator<Item = (bool, Timestamp, u8, u8)>>(
    //     u: &mut Unstructured,
    //     hs: H,
    // ) -> Vec<Action> {
    //     hs.map(|(e, t, b, w)| {
    //         if e {
    //             let mut h = Create::arbitrary(u).unwrap();
    //             h.weight = EntryRateWeight {
    //                 bucket_id: b,
    //                 units: w,
    //                 rate_bytes: 0,
    //             };
    //             h.timestamp = t;
    //             h.into()
    //         } else {
    //             let mut h = CreateLink::arbitrary(u).unwrap();
    //             h.weight = LinkWeight {
    //                 bucket_id: b,
    //                 units: w,
    //             };
    //             h.timestamp = t;
    //             h.into()
    //         }
    //     })
    //     .collect()
    // }
}
