use std::num::NonZeroU32;

use governor::Quota;

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
pub struct BandwidthThrottle {
    inbound: Option<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    outbound: Option<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl BandwidthThrottle {
    /// Set the inbound and outbound bandwidth limits in megabits per second.
    pub(super) fn new(inbound_mbps: f64, outbound_mbps: f64) -> Self {
        // Convert to bits per second.
        let inbound_bps = inbound_mbps * 1000.0 * 1000.0;
        let outbound_bps = outbound_mbps * 1000.0 * 1000.0;
        // Double the max message size to allow room for padding.
        let max_burst_bits =
            NonZeroU32::new(MAX_SEND_BUF_BYTES as u32 * 8 * 2).expect("This can't be zero");

        let inbound = NonZeroU32::new(inbound_bps as u32).map(|inbound_bps| {
            RateLimiter::direct(Quota::per_second(inbound_bps).allow_burst(max_burst_bits))
        });

        let outbound = NonZeroU32::new(outbound_bps as u32).map(|outbound_bps| {
            RateLimiter::direct(Quota::per_second(outbound_bps).allow_burst(max_burst_bits))
        });
        Self { inbound, outbound }
    }

    /// Wait until there's enough bandwidth to send this many bytes.
    pub(super) async fn outgoing_bytes(&self, bytes: usize) {
        if let Some(bits) = NonZeroU32::new(bytes as u32 * 8) {
            if let Some(outbound) = &self.outbound {
                if outbound.until_n_ready(bits).await.is_err() {
                    tracing::error!("Tried to send a message larger than the max message size");
                }
            }
        }
    }

    /// Wait until there's enough bandwidth to receive this many bytes.
    pub(super) async fn incoming_bytes(&self, bytes: usize) {
        if let Some(bits) = NonZeroU32::new(bytes as u32 * 8) {
            if let Some(inbound) = &self.inbound {
                if inbound.until_n_ready(bits).await.is_err() {
                    tracing::error!("Tried to receive a message larger than the max message size");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_limiter() {
        let bandwidth = BandwidthThrottle::new(0.1, 0.1);
        let bytes = MAX_SEND_BUF_BYTES;
        // Hit the burst limit.
        bandwidth.outgoing_bytes(bytes).await;
        bandwidth.outgoing_bytes(bytes).await;
        let mut count = 0;

        // Now we will be limited to 0.1 mbps.
        let now = std::time::Instant::now();
        for _ in 0..5 {
            bandwidth.outgoing_bytes(bytes).await;
            count += bytes;
        }
        let seconds = now.elapsed().as_secs();
        let megabits = (count * 8) as f64 / 1_000_000.0;
        let mbps = megabits / seconds as f64;
        // Allow for small rounding error.
        assert!(mbps < 0.11);
    }
}
