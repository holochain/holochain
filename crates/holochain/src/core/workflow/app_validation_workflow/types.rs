use std::convert::TryFrom;

use holo_hash::AnyDhtHash;

use crate::core::validation::OutcomeOrError;

use super::AppValidationOutcome;

#[derive(Debug)]
/// The outcome of sys validation
pub enum Outcome {
    /// Moves to integration
    Accepted,
    /// Stays in limbo because a
    /// dependency needs is required to validate
    /// and could not be found
    AwaitingDeps(Vec<AnyDhtHash>),
    /// Moves to integration with status rejected
    Rejected(String),
}

impl Outcome {
    pub fn awaiting<E, I: Into<AnyDhtHash> + Clone>(h: &I) -> OutcomeOrError<Self, E> {
        OutcomeOrError::Outcome(Outcome::AwaitingDeps(vec![h.clone().into()]))
    }
    /// Early exits with an accepted outcome
    pub fn accepted<T>() -> AppValidationOutcome<T> {
        Err(OutcomeOrError::Outcome(Outcome::Accepted))
    }
}

impl<E> TryFrom<OutcomeOrError<Outcome, E>> for Outcome {
    type Error = E;

    fn try_from(value: OutcomeOrError<Outcome, E>) -> Result<Self, Self::Error> {
        match value {
            OutcomeOrError::Outcome(o) => Ok(o),
            OutcomeOrError::Err(e) => Err(e),
        }
    }
}
