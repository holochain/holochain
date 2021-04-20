//! Wrapper type to indicate some data which has a ValidationStatus associated
//! with it.
//!
//! This type indicates that the use of the underlying data is tied to a
//! ValidationStatus related to this data. It's meant to force you to think
//! about the validity of this piece of data and assign a status.
//!
//! The meaning of using this type is context-specific, but in general it means:
//! "this data is available in this context because an authority produced it,
//! and the validation status is the status of the DhtOp which that authority
//! holds".

use crate::ValidationStatus;
use holochain_serialized_bytes::prelude::*;

#[derive(
    Clone, Debug, PartialEq, Eq, Hash, derive_more::From, derive_more::Into, Serialize, Deserialize,
)]
/// Data with an optional validation status.
pub struct Judged<T> {
    /// The data that the status applies to.
    pub data: T,
    /// The validation status of the data.
    status: Option<ValidationStatus>,
}

impl<T> Judged<T> {
    /// Create a Judged item with a given ValidationStatus
    pub fn new(data: T, status: ValidationStatus) -> Self {
        Self {
            data,
            status: Some(status),
        }
    }

    /// Create a Judged item where it's ok to not have a status.
    pub fn raw(data: T, status: Option<ValidationStatus>) -> Self {
        Self { data, status }
    }

    /// Create a valid status of T.
    pub fn valid(data: T) -> Self {
        Self {
            data,
            status: Some(ValidationStatus::Valid),
        }
    }

    /// Create a status where T hasn't been validated.
    pub fn none(data: T) -> Self {
        Self { data, status: None }
    }

    /// Move out the inner data type
    pub fn into_data(self) -> T {
        self.data
    }

    /// Map this type to another judged type with the same status.
    pub fn map<B, F>(self, f: F) -> Judged<B>
    where
        F: FnOnce(T) -> B,
    {
        Judged::<B> {
            data: f(self.data),
            status: self.status,
        }
    }
}

/// Data that requires a validation status.
pub trait HasValidationStatus {
    /// The type of the inner data
    type Data;

    /// Get the status of a some data.
    /// None means this data has not been validated yet.
    fn validation_status(&self) -> Option<ValidationStatus>;

    /// The data which has the validation status
    fn data(&self) -> &Self::Data;
}

impl<T> HasValidationStatus for Judged<T> {
    type Data = T;

    fn validation_status(&self) -> Option<ValidationStatus> {
        self.status
    }

    fn data(&self) -> &Self::Data {
        &self.data
    }
}
