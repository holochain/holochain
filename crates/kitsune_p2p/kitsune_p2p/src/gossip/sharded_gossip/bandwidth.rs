use std::{
    num::NonZeroU32,
    sync::atomic::{AtomicU64, AtomicUsize},
};

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
            tuning_params.gossip_burst_ratio,
        );
        let historic = BandwidthThrottle::new(
            tuning_params.gossip_historic_inbound_target_mbps,
            tuning_params.gossip_historic_outbound_target_mbps,
            tuning_params.gossip_burst_ratio,
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
    start_time: Instant,
    bits_inbound: AtomicUsize,
    peak_inbound: AtomicUsize,
    bits_outbound: AtomicUsize,
    peak_outbound: AtomicUsize,
    last_inbound_time: AtomicU64,
    last_outbound_time: AtomicU64,
}

impl BandwidthThrottle {
    /// Set the inbound and outbound bandwidth limits in megabits per second.
    pub fn new(inbound_mbps: f64, outbound_mbps: f64, burst_ratio: f64) -> Self {
        Self::new_inner(
            inbound_mbps,
            outbound_mbps,
            burst_ratio,
            governor::clock::DefaultClock::default(),
        )
    }
}

#[cfg(test)]
impl BandwidthThrottle<governor::clock::FakeRelativeClock> {
    fn test(
        inbound_mbps: f64,
        outbound_mbps: f64,
        burst_ratio: f64,
        clock: governor::clock::FakeRelativeClock,
    ) -> Self {
        Self::new_inner(inbound_mbps, outbound_mbps, burst_ratio, clock)
    }
}

impl<C> BandwidthThrottle<C>
where
    C: Clock,
{
    fn new_inner(inbound_mbps: f64, outbound_mbps: f64, burst_ratio: f64, clock: C) -> Self {
        // Convert to bits per second.
        let inbound_bps = inbound_mbps * 1000.0 * 1000.0;
        let outbound_bps = outbound_mbps * 1000.0 * 1000.0;

        let inbound = NonZeroU32::new(inbound_bps as u32).map(|bps| {
            let burst = NonZeroU32::new((inbound_bps * burst_ratio) as u32)
                .expect("burst_ratio cannot be 0");
            RateLimiter::direct_with_clock(Quota::per_second(bps).allow_burst(burst), &clock)
        });

        let outbound = NonZeroU32::new(outbound_bps as u32).map(|bps| {
            let burst = NonZeroU32::new((outbound_bps * burst_ratio) as u32)
                .expect("burst_ratio cannot be 0");
            RateLimiter::direct_with_clock(Quota::per_second(bps).allow_burst(burst), &clock)
        });
        Self {
            clock,
            inbound,
            outbound,
            start_time: Instant::now(),
            bits_inbound: AtomicUsize::new(0),
            peak_inbound: AtomicUsize::new(0),
            bits_outbound: AtomicUsize::new(0),
            peak_outbound: AtomicUsize::new(0),
            last_inbound_time: AtomicU64::new(0),
            last_outbound_time: AtomicU64::new(0),
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
                        governor::NegativeMultiDecision::InsufficientCapacity(mut cap) => {
                            tracing::error!(
                                "Tried to send {} bits, which is larger than the maximum possible of {} bits. Allowing this large message through anyway!", bits, cap
                            );
                            // TODO: rather than allowing this message through, we should bubble this error up so that the sender can split
                            // the message into smaller chunks. We don't easily have that capacity right now, so, better to violate rate
                            // limiting than to go into an infinite loop...

                            // Drain the rate limiter's capacity completely, to be as accurate as possible.
                            // (ideally we would just drain the capacity completely in one fell swoop, but `governor`'s API does not allow this.)
                            while cap > 1 {
                                outbound
                                    .check_n(unsafe { NonZeroU32::new_unchecked(cap) })
                                    .ok();
                                cap /= 2;
                            }
                            break;
                        }
                    }
                }
            }
            let el = self.start_time.elapsed();
            let last_s = self
                .last_outbound_time
                .swap(el.as_secs(), std::sync::atomic::Ordering::Relaxed);
            let total_bits = self
                .bits_outbound
                .fetch_add(bits.get() as usize, std::sync::atomic::Ordering::Relaxed)
                + bits.get() as usize;
            let bps = total_bits
                .checked_div(el.as_secs() as usize)
                .unwrap_or_default();
            let current_bps = (bits.get() as u64).checked_div(last_s).unwrap_or_default();
            let max_bps = self
                .peak_outbound
                .fetch_max(bps, std::sync::atomic::Ordering::Relaxed)
                .max(bps);
            let s = tracing::trace_span!("bandwidth");
            s.in_scope(|| {
                tracing::trace!(
                    "Outbound current: {}bps {:.2}mbps, average: {}bps {:.2}mbps, max: {}bps {:.2}mbps",
                    current_bps,
                    current_bps as f64 / 1_048_576.0,
                    bps,
                    bps as f64 / 1_048_576.0,
                    max_bps,
                    max_bps as f64 / 1_048_576.0
                )
            })
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
            let el = self.start_time.elapsed();
            let last_s = self
                .last_inbound_time
                .swap(el.as_secs(), std::sync::atomic::Ordering::Relaxed);
            let total_bits = self
                .bits_inbound
                .fetch_add(bits.get() as usize, std::sync::atomic::Ordering::Relaxed)
                + bits.get() as usize;
            let bps = total_bits
                .checked_div(el.as_secs() as usize)
                .unwrap_or_default();
            let current_bps = (bits.get() as u64).checked_div(last_s).unwrap_or_default();
            let max_bps = self
                .peak_inbound
                .fetch_max(bps, std::sync::atomic::Ordering::Relaxed)
                .max(bps);
            let s = tracing::trace_span!("bandwidth");
            s.in_scope(|| {
                tracing::trace!(
                    "Inbound current: {}bps {:.2}mbps, average: {}bps {:.2}mbps, max: {}bps {:.2}mbps",
                    current_bps,
                    current_bps as f64 / 1_000_000.0,
                    bps,
                    bps as f64 / 1_000_000.0,
                    max_bps,
                    max_bps as f64 / 1_000_000.0
                )
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_limiter() {
        observability::test_run().ok();
        let clock = governor::clock::FakeRelativeClock::default();
        // max * 2 * 8 = 0.1 * 1_000_000 * burst_ratio => burst_ratio = max * 2 * 8 / 0.1 / 1_000_000
        let burst_ratio = MAX_SEND_BUF_BYTES as f64 * 2.0 * 8.0 / 1_000_000.0 / 0.1;
        assert_eq!(burst_ratio, 2560.0);
        let bandwidth = BandwidthThrottle::test(0.1, 0.1, burst_ratio, clock.clone());
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
            // When we advance the clock 1 second less than the required time
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
