/// The minimum number of peers before sharding can begin.
/// This factors in the expected uptime to reach the redundancy target.
pub const DEFAULT_MIN_PEERS: usize = (DEFAULT_REDUNDANCY_TARGET as f64 / DEFAULT_UPTIME) as usize;

/// The minimum number of peers we can consider acceptable to see in our arc
/// during testing.
pub const DEFAULT_MIN_REDUNDANCY: u32 = (REDUNDANCY_FLOOR as f64 / DEFAULT_UPTIME) as u32;

/// Number of copies of a given hash available at any given time.
pub(crate) const DEFAULT_REDUNDANCY_TARGET: usize = 50;

/// Default assumed up time for nodes.
pub(crate) const DEFAULT_UPTIME: f64 = 0.5;

/// If the redundancy drops due to inaccurate estimation we can't
/// go lower then this level of redundancy.
/// Note this can only be tested and not proved.
pub(crate) const REDUNDANCY_FLOOR: usize = 20;

/// Margin of error for floating point comparisons
pub(crate) const ERROR_MARGIN: f64 = 0.0000000001;
