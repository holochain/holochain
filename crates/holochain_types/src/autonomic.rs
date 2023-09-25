//! Holochain autonomic type helpers.

/// The various processes which run "autonomically", aka subconsciously.
/// These are currently not used.
pub enum AutonomicProcess {
    /// Validation / Correction may propagate much slower.
    SlowHeal,

    /// See how many validators we can find on the network for all of our entries
    /// Push out new hold requests if the health is too low.
    HealthCheck,
}
