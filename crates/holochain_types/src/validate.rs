//! the _host_ types used to track the status/result of validating entries
//! c.f. _guest_ types for validation callbacks and packages across the wasm boudary in zome_types

use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::prelude::*;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    SerializedBytes,
    derive_more::From,
    derive_more::Into,
)]
/// Type for sending responses to `get_validation_package`
pub struct ValidationPackageResponse(pub Option<ValidationPackage>);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Data with an optional validation status.
pub struct ValStatusOf<T> {
    /// The that the status applies to.
    pub data: T,
    /// The validation status of the data.
    pub status: Option<ValidationStatus>,
}

impl<T> ValStatusOf<T> {
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

impl<T> HasValidationStatus for ValStatusOf<T> {
    type Data = T;

    fn validation_status(&self) -> Option<ValidationStatus> {
        self.status
    }

    fn data(&self) -> &Self::Data {
        &self.data
    }
}
