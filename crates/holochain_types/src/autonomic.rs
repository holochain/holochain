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

/// A cue that the autonomic system should perform one of its functions now,
/// rather than at the next scheduled time
pub enum AutonomicCue {
    // /// Cue sent when it is known that entries are ready for initial publishing,
    // /// i.e. after committing new entries to your source chain
    // Publish(Address),
}

impl From<AutonomicCue> for AutonomicProcess {
    fn from(cue: AutonomicCue) -> AutonomicProcess {
        match cue {}
    }
}
