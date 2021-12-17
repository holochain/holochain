use std::num::NonZeroU32;

use governor::{clock::Clock, Quota};

use super::*;

#[derive(Clone)]
/// Set of bandwidth throttles for all gossip loops.
pub struct BandwidthThrottles {
    recent: Arc<BandwidthThrottle>,
    historic: Arc<BandwidthThrottle>,
}

impl BandwidthThrottles {
    /// Create a new set of throttles from the configuration.
    pub fn new(tuning_params: &KitsuneP2pTuningParams) -> Self {
        let recent = BandwidthThrottle::new(
            tuning_params.gossip_inbound_target_mbps,
            tuning_params.gossip_outbound_target_mbps,
        );
        let historic = BandwidthThrottle::new(
            tuning_params.gossip_historic_inbound_target_mbps,
            tuning_params.gossip_historic_outbound_target_mbps,
        );
        Self {
            recent: Arc::new(recent),
            historic: Arc::new(historic),
        }
    }

    /// Get the throttle for the recent loop.
    pub fn recent(&self) -> Arc<BandwidthThrottle> {
        self.recent.clone()
    }

    /// Get the throttle for the historical loop.
    pub fn historical(&self) -> Arc<BandwidthThrottle> {
        self.historic.clone()
    }
}

/// Manages incoming and outgoing bandwidth by providing methods which
/// asynchronously wait for enough bandwidth to become available before
/// processing a chunk of bytes
pub struct BandwidthThrottle<C = DefaultClock>
where
    C: Clock,
{
    clock: C,
    inbound: Option<RateLimiter<NotKeyed, InMemoryState, C>>,
    outbound: Option<RateLimiter<NotKeyed, InMemoryState, C>>,
}

impl BandwidthThrottle {
    /// Set the inbound and outbound bandwidth limits in megabits per second.
    pub fn new(inbound_mbps: f64, outbound_mbps: f64) -> Self {
        Self::new_inner(
            inbound_mbps,
            outbound_mbps,
            governor::clock::DefaultClock::default(),
        )
    }
}

#[cfg(test)]
impl BandwidthThrottle<governor::clock::FakeRelativeClock> {
    fn test(
        inbound_mbps: f64,
        outbound_mbps: f64,
        clock: governor::clock::FakeRelativeClock,
    ) -> Self {
        Self::new_inner(inbound_mbps, outbound_mbps, clock)
    }
}

impl<C> BandwidthThrottle<C>
where
    C: Clock,
{
    fn new_inner(inbound_mbps: f64, outbound_mbps: f64, clock: C) -> Self {
        // Convert to bits per second.
        let inbound_bps = inbound_mbps * 1000.0 * 1000.0;
        let outbound_bps = outbound_mbps * 1000.0 * 1000.0;
        // Double the max message size to allow room for padding.
        let max_burst_bits =
            NonZeroU32::new(MAX_SEND_BUF_BYTES as u32 * 8 * 2).expect("This can't be zero");

        let inbound = NonZeroU32::new(inbound_bps as u32).map(|inbound_bps| {
            RateLimiter::direct_with_clock(
                Quota::per_second(inbound_bps).allow_burst(max_burst_bits),
                &clock,
            )
        });

        let outbound = NonZeroU32::new(outbound_bps as u32).map(|outbound_bps| {
            RateLimiter::direct_with_clock(
                Quota::per_second(outbound_bps).allow_burst(max_burst_bits),
                &clock,
            )
        });
        Self {
            clock,
            inbound,
            outbound,
        }
    }

    /// Wait until there's enough bandwidth to send this many bytes.
    pub async fn outgoing_bytes(&self, bytes: usize) {
        if let Some(bits) = NonZeroU32::new(bytes as u32 * 8) {
            if let Some(outbound) = &self.outbound {
                while let Err(e) = outbound.check_n(bits) {
                    match e {
                        governor::NegativeMultiDecision::BatchNonConforming(_, n) => {
                            let dur = n.wait_time_from(governor::clock::Clock::now(&self.clock));
                            if dur.as_secs() > 1 {
                                tracing::info!(
                                    "Waiting {:?} to send {} bits, {} bytes",
                                    dur,
                                    bits,
                                    bytes
                                );
                            }
                            tokio::time::sleep(dur).await;
                        }
                        governor::NegativeMultiDecision::InsufficientCapacity(_) => {
                            tracing::error!(
                                "Tried to send a message larger than the max message size"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Wait until there's enough bandwidth to receive this many bytes.
    pub async fn incoming_bytes(&self, bytes: usize) {
        if let Some(bits) = NonZeroU32::new(bytes as u32 * 8) {
            if let Some(inbound) = &self.inbound {
                while let Err(e) = inbound.check_n(bits) {
                    match e {
                        governor::NegativeMultiDecision::BatchNonConforming(_, n) => {
                            let dur = n.wait_time_from(governor::clock::Clock::now(&self.clock));
                            if dur.as_secs() > 1 {
                                tracing::info!(
                                    "Waiting {:?} to receive {} bits, {} bytes",
                                    dur,
                                    bits,
                                    bytes
                                );
                            }
                            tokio::time::sleep(dur).await;
                        }
                        governor::NegativeMultiDecision::InsufficientCapacity(_) => {
                            tracing::error!(
                                "Tried to receive a message larger than the max message size"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_limiter() {
        let clock = governor::clock::FakeRelativeClock::default();
        let bandwidth = BandwidthThrottle::test(0.1, 0.1, clock.clone());
        let bytes = MAX_SEND_BUF_BYTES;
        // Hit the burst limit.
        bandwidth.outgoing_bytes(MAX_SEND_BUF_BYTES).await;
        bandwidth.outgoing_bytes(MAX_SEND_BUF_BYTES).await;
        let mut count = 0;

        // Now we will be limited to 0.1 mbps.
        let mut seconds = 0;
        for _ in 0..5 {
            let megabits = (bytes * 8) as f64 / 1_000_000.0;
            let time = megabits / 0.1;
            let advance_by = Duration::from_secs(time as u64 - 1);
            seconds += advance_by.as_nanos();
            clock.advance(advance_by);
            let r = tokio::time::timeout(Duration::from_secs(10), bandwidth.outgoing_bytes(bytes))
                .await;
            // When we advance the clock 1 second less then the required time
            // the outgoing bytes times out because the clock is set to just before
            // enough time to send the bytes
            assert!(r.is_err());

            let advance_by = Duration::from_secs(1);
            seconds += advance_by.as_nanos();
            clock.advance(advance_by);
            let n = tokio::time::Instant::now();
            bandwidth.outgoing_bytes(bytes).await;
            // Now we advance the clock and the function returns
            // immediately.
            assert!(n.elapsed().is_zero());
            count += bytes;
        }
        let megabits = (count * 8) as f64 / 1_000_000.0;
        let mbps = megabits / seconds as f64;
        // Allow for small rounding error.
        assert!(mbps < 0.11);
    }
}
