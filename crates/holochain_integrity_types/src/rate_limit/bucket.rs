use kitsune_p2p_timestamp::Timestamp;

use crate::Header;

/// Defines parameters for a "bucket algorithm" for throttling agent activity.
/// This struct defines such a "bucket".
///
/// The bucket has a certain fixed capacity, and every `refill_interval_ms`,
/// its capacity is refilled by `refill_amount`. When it reaches its `capacity`,
/// no more refilling can occur.
///
/// Each [`Element`] goes through the `weigh` guest callback function, which
/// assigns it a bucket and a weight. The weight determines how many units of
/// capacity is removed by the authoring of that Element. If the capacity of
/// the bucket would go below zero, then that action is not allowed, and the
/// agent must wait until the bucket is sufficiently refilled.
pub struct RateLimit {
    capacity: u32,
    refill_interval_ms: u32,
    refill_amount: u32,
}

impl RateLimit {
    /// Process an item, letting the bucket state change
    pub fn change(&mut self, weight: u8, timestamp: Timestamp) -> bool {
        // self.capacity += todo!();
        todo!();
    }
}

/// An indexable collection of [`RateLimit`] buckets
pub struct RateLimits(Vec<RateLimit>);

impl RateLimits {
    /// Process an item, letting the bucket states change
    pub fn change(&mut self, header: &Header) -> Result<(), String> {
        let weight = header.rate_data();
        self.0
            .get_mut(weight.rate_bucket as usize)
            .map(|b| b.change(weight.rate_weight, header.timestamp()))
            .ok_or_else(|| format!("No rate bucket at index {}", weight.rate_bucket))?
            .then(|| ())
            .ok_or_else(|| {
                format!(
                    "Rate bucket at index {} is empty! Rate limit exceeded.",
                    weight.rate_bucket
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use arbitrary::*;
    use kitsune_p2p_timestamp::Timestamp;

    use crate::{Create, CreateLink, EntryWeight, Header, LinkWeight};

    fn make_headers<H: Iterator<Item = (bool, Timestamp, u8, u8)>>(
        u: &mut Unstructured,
        hs: H,
    ) -> Vec<Header> {
        hs.map(|(e, t, b, w)| {
            if e {
                let mut h = Create::arbitrary(u).unwrap();
                h.weight = EntryWeight {
                    rate_bucket: b,
                    rate_weight: w,
                    rate_bytes: 0,
                };
                h.timestamp = t;
                h.into()
            } else {
                let mut h = CreateLink::arbitrary(u).unwrap();
                h.weight = LinkWeight {
                    rate_bucket: b,
                    rate_weight: w,
                };
                h.timestamp = t;
                h.into()
            }
        })
        .collect()
    }

    #[test]
    fn empty_bucket() {
        todo!()
    }
}
