use std::collections::HashSet;

use holochain_zome_types::validate::ValidationStatus;

#[derive(Debug, Clone, derive_more::Constructor, derive_more::From)]
pub struct DisputedStatus {
    status: HashSet<ValidationStatus>,
}

impl DisputedStatus {
    /// Resolve a potentially disputed status by
    /// considering everything except a set with only valid as invalid.
    pub fn resolve(&self) -> Option<ValidationStatus> {
        // A set with only valid
        if self.status.len() == 1 && self.status.contains(&ValidationStatus::Valid) {
            Some(ValidationStatus::Valid)
        // Choose the first invalid status or None if it's empty
        } else {
            self.status
                .iter()
                .find(|v| **v != ValidationStatus::Valid)
                .copied()
        }
    }

    /// Resolves the status considering an absence
    /// of status as valid.
    pub fn is_valid(&self) -> bool {
        matches!(self.resolve(), Some(ValidationStatus::Valid) | None)
    }
}
