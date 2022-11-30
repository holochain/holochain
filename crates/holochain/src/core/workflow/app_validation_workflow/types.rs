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
    /// Helper function for creating awaiting deps and exiting
    /// when the dependency isn't found
    pub fn awaiting<E, I: Into<AnyDhtHash> + Clone>(h: &I) -> OutcomeOrError<Self, E> {
        OutcomeOrError::Outcome(Outcome::AwaitingDeps(vec![h.clone().into()]))
    }
    /// Helper function for creating rejected outcomes
    pub fn rejected<E, I: Into<String>>(s: I) -> OutcomeOrError<Self, E> {
        OutcomeOrError::Outcome(Outcome::Rejected(s.into()))
    }
    /// Exit early with an awaiting outcome
    pub fn exit_with_awaiting<T, I: Into<AnyDhtHash>, It: IntoIterator<Item = I>>(
        h: It,
    ) -> AppValidationOutcome<T> {
        Err(OutcomeOrError::Outcome(Outcome::AwaitingDeps(
            h.into_iter().map(Into::into).collect(),
        )))
    }
    /// Early exits with an accepted outcome
    pub fn accepted<T>() -> AppValidationOutcome<T> {
        Err(OutcomeOrError::Outcome(Outcome::Accepted))
    }
    /// Exit early with a rejected outcome
    pub fn exit_with_rejected<T, I: Into<String>>(reason: I) -> AppValidationOutcome<T> {
        Err(OutcomeOrError::Outcome(Outcome::Rejected(reason.into())))
    }
}

/// Turn the OutcomeOrError into an Outcome or an Error
impl<E> TryFrom<OutcomeOrError<Outcome, E>> for Outcome {
    type Error = E;

    fn try_from(value: OutcomeOrError<Outcome, E>) -> Result<Self, Self::Error> {
        match value {
            OutcomeOrError::Outcome(o) => Ok(o),
            OutcomeOrError::Err(e) => Err(e),
        }
    }
}
