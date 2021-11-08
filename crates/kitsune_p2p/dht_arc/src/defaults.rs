/// The minimum number of peers before sharding can begin.
/// This factors in the expected uptime to reach the redundancy target.
pub const DEFAULT_MIN_PEERS: usize = (DEFAULT_REDUNDANCY_TARGET as f64 / DEFAULT_UPTIME) as usize;

/// The minimum number of peers we can consider acceptable to see in our arc
/// during testing.
pub const DEFAULT_MIN_REDUNDANCY: u32 = (REDUNDANCY_FLOOR as f64 / DEFAULT_UPTIME) as u32;

/// Number of copies of a given hash available at any given time.
pub(crate) const DEFAULT_REDUNDANCY_TARGET: usize = 50;

/// Establish an upper target, this much higher than the lower target of coverage.
pub(crate) const DEFAULT_COVERAGE_BUFFER: f64 = 0.05; // 5%

/// Default assumed up time for nodes.
pub(crate) const DEFAULT_UPTIME: f64 = 0.5;

/// Due to estimation noise we don't want a very small difference
/// between observed coverage and estimated coverage to
/// amplify when scaled to by the estimated total peers.
/// This threshold must be reached before an estimated coverage gap
/// is calculated.
pub(crate) const DEFAULT_NOISE_THRESHOLD: f64 = 0.01;

/// The amount "change in arc" is scaled to prevent rapid changes.
/// This also represents the maximum coverage change in a single update
/// as a difference of 1.0 would scale to 0.2.
pub(crate) const DEFAULT_DELTA_SCALE: f64 = 0.2;

/// The minimal "change in arc" before we stop scaling.
/// This prevents never reaching the target arc coverage.
pub(crate) const DEFAULT_DELTA_THRESHOLD: f64 = 0.01;

/// If the redundancy drops due to inaccurate estimation we can't
/// go lower then this level of redundancy.
/// Note this can only be tested and not proved.
pub(crate) const REDUNDANCY_FLOOR: usize = 20;

/// Margin of error for floating point comparisons
pub(crate) const ERROR_MARGIN: f64 = 0.0000000001;
