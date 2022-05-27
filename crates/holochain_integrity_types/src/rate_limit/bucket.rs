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

#[cfg(test)]
mod tests {

    #[test]
    fn empty_bucket() {
        todo!()
    }
}
