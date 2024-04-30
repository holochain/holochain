//! Types needed for all validation
use std::convert::TryFrom;

use super::workflow::WorkflowResult;
use super::SourceChainError;
use super::SysValidationError;
use super::ValidationOutcome;

/// Exit early with either an outcome or an error
#[derive(Debug)]
pub enum OutcomeOrError<T, E> {
    Outcome(T),
    Err(E),
}

impl<T, E> OutcomeOrError<T, E> {
    /// Peel off an Outcome if that's what it is
    pub fn into_outcome(self) -> Option<T> {
        if let Self::Outcome(t) = self {
            Some(t)
        } else {
            None
        }
    }

    /// Peel off an Err if that's what it is
    pub fn into_err(self) -> Option<E> {
        if let Self::Err(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

/// Helper macro for implementing from sub error types
/// for the error in OutcomeOrError
#[macro_export]
macro_rules! from_sub_error {
    ($error_type:ident, $sub_error_type:ident) => {
        impl<T> From<$sub_error_type> for OutcomeOrError<T, $error_type> {
            fn from(e: $sub_error_type) -> Self {
                OutcomeOrError::Err($error_type::from(e))
            }
        }
    };
}

impl OutcomeOrError<ValidationOutcome, SysValidationError> {
    /// Convert an OutcomeOrError<ValidationOutcome, SysValidationError> into
    /// a InvalidCommit and exit the call zome workflow early
    pub fn invalid_call_zome_commit<T>(self) -> WorkflowResult<T> {
        Err(SourceChainError::InvalidCommit(ValidationOutcome::try_from(self)?.to_string()).into())
    }
}
