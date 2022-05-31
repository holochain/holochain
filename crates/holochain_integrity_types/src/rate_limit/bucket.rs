use std::{sync::Arc, time::Duration};

use kitsune_p2p_timestamp::Timestamp;

use crate::{
    Header, RateBucketCapacity, RateBucketError, RateBucketId, RateBucketResult, RateWeight,
};

/// Defines parameters for a "bucket algorithm" for throttling agent activity.
/// This struct defines such a "bucket".
///
/// Each bucket has a certain fixed capacity, and starts out empty (level=0).
/// Each Header has a bucket and weight assigned to it, and when that Header is
/// authored, it adds an amount of units equal to is weight to its assigned bucket.
/// If a Header causes a bucket's level to exceed its `capacity`, that Header
/// is invalid.
///
/// Every `drain_interval_ms`, the bucket's level is reduced by `drain_amount`.
/// In other words, the bucket is "draining" at a rate of
/// `drain_amount / drain_interval_ms`.
#[derive(Debug, PartialEq, Eq)]
pub struct RateLimit {
    capacity: RateBucketCapacity,
    drain_amount: RateBucketCapacity,
    drain_interval_ms: u32,
}

impl RateLimit {
    /// Process an item, letting the bucket state change
    pub fn change(&mut self, weight: u8, timestamp: Timestamp) -> bool {
        // self.capacity += todo!();
        todo!();
    }
}

/// Tracks the current level of a bucket, and calculates the next level given
/// an incoming header. See [`RateLimit`] for details on the bucket algorithm.
#[derive(Debug, PartialEq, Eq)]
pub struct BucketState {
    params: Arc<RateLimit>,
    index: RateBucketId,
    level: RateBucketCapacity,
    last_access: Option<Timestamp>,
}

impl BucketState {
    /// Constructor
    pub fn new(params: Arc<RateLimit>, index: RateBucketId) -> Self {
        Self {
            params,
            index,
            level: 0,
            last_access: None,
        }
    }

    /// Process an item with the given weight at the given time.
    /// Bucket overflow or nonmonotonic timestamp causes an error.
    pub fn change(&mut self, weight: RateWeight, timestamp: Timestamp) -> RateBucketResult<()> {
        let drain_amount = if let Some(last_access) = self.last_access {
            if timestamp <= last_access {
                Err(RateBucketError::NonMonotonicTimestamp(
                    last_access,
                    timestamp,
                ))?
            }
            let interval = (timestamp - last_access)?
                .num_milliseconds()
                .min(525600 * 60 * 1000) as u32; // 1 year
            self.params.drain_amount.saturating_mul(interval) / self.params.drain_interval_ms
        } else {
            self.params.capacity
        };
        self.last_access = Some(timestamp);
        self.level = self.level.saturating_sub(drain_amount) + weight as RateBucketCapacity;
        if self.level > self.params.capacity {
            Err(RateBucketError::BucketOverflow(self.index))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Create, CreateLink, EntryWeight, Header, LinkWeight};
    use arbitrary::*;
    use kitsune_p2p_timestamp::Timestamp;

    #[test]
    fn bucket_drains_completely() {
        let p = Arc::new(RateLimit {
            capacity: 100,
            drain_amount: 10,
            drain_interval_ms: 1,
        });
        let mil = 1000;
        let mut b = BucketState::new(p, 0);
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
        let mut b = BucketState::new(p, 0);
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
        let mut b = BucketState::new(p, 0);

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
            Err(RateBucketError::BucketOverflow(0))
        );
    }

    // fn make_headers<H: Iterator<Item = (bool, Timestamp, u8, u8)>>(
    //     u: &mut Unstructured,
    //     hs: H,
    // ) -> Vec<Header> {
    //     hs.map(|(e, t, b, w)| {
    //         if e {
    //             let mut h = Create::arbitrary(u).unwrap();
    //             h.weight = EntryWeight {
    //                 rate_bucket: b,
    //                 rate_weight: w,
    //                 rate_bytes: 0,
    //             };
    //             h.timestamp = t;
    //             h.into()
    //         } else {
    //             let mut h = CreateLink::arbitrary(u).unwrap();
    //             h.weight = LinkWeight {
    //                 rate_bucket: b,
    //                 rate_weight: w,
    //             };
    //             h.timestamp = t;
    //             h.into()
    //         }
    //     })
    //     .collect()
    // }
}
