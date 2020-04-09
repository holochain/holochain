//! This module contains Error type definitions that are used throughout persistence.

use self::PersistenceError::*;
use futures::channel::oneshot::Canceled as FutureCanceled;
use holochain_json_api::{error::JsonError, json::*};
use serde_json::Error as SerdeError;
use std::{
    error::Error,
    fmt,
    io::{self, Error as IoError},
    option::NoneError,
};

//--------------------------------------------------------------------------------------------------
// PersistenceError
//--------------------------------------------------------------------------------------------------

#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, DefaultJson, Hash, PartialOrd, Ord,
)]
pub enum PersistenceError {
    ErrorGeneric(String),
    IoError(String),
    SerializationError(String),
}

impl PersistenceError {
    pub fn new(msg: &str) -> PersistenceError {
        PersistenceError::ErrorGeneric(msg.to_string())
    }
}

impl From<JsonError> for PersistenceError {
    fn from(json_error: JsonError) -> PersistenceError {
        match json_error {
            JsonError::ErrorGeneric(s) => PersistenceError::ErrorGeneric(s),
            JsonError::IoError(s) => PersistenceError::IoError(s),
            JsonError::SerializationError(s) => PersistenceError::SerializationError(s),
        }
    }
}
pub type PersistenceResult<T> = Result<T, PersistenceError>;

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ErrorGeneric(err_msg) => write!(f, "{}", err_msg),
            SerializationError(err_msg) => write!(f, "{}", err_msg),
            IoError(err_msg) => write!(f, "{}", err_msg),
        }
    }
}

impl Error for PersistenceError {}

impl From<PersistenceError> for String {
    fn from(holochain_persistence_error: PersistenceError) -> Self {
        holochain_persistence_error.to_string()
    }
}

impl From<String> for PersistenceError {
    fn from(error: String) -> Self {
        PersistenceError::new(&error)
    }
}

impl From<&'static str> for PersistenceError {
    fn from(error: &str) -> Self {
        PersistenceError::new(error)
    }
}

/// standard strings for std io errors
fn reason_for_io_error(error: &IoError) -> String {
    match error.kind() {
        io::ErrorKind::InvalidData => format!("contains invalid data: {}", error),
        io::ErrorKind::PermissionDenied => format!("missing permissions to read: {}", error),
        _ => format!("unexpected error: {}", error),
    }
}

impl<T> From<::std::sync::PoisonError<T>> for PersistenceError {
    fn from(error: ::std::sync::PoisonError<T>) -> Self {
        PersistenceError::ErrorGeneric(format!("sync poison error: {}", error))
    }
}

impl From<IoError> for PersistenceError {
    fn from(error: IoError) -> Self {
        PersistenceError::IoError(reason_for_io_error(&error))
    }
}

impl From<SerdeError> for PersistenceError {
    fn from(error: SerdeError) -> Self {
        PersistenceError::SerializationError(error.to_string())
    }
}

impl From<base64::DecodeError> for PersistenceError {
    fn from(error: base64::DecodeError) -> Self {
        PersistenceError::ErrorGeneric(format!("base64 decode error: {}", error.to_string()))
    }
}

impl From<std::str::Utf8Error> for PersistenceError {
    fn from(error: std::str::Utf8Error) -> Self {
        PersistenceError::ErrorGeneric(format!("std::str::Utf8Error error: {}", error.to_string()))
    }
}

impl From<FutureCanceled> for PersistenceError {
    fn from(_: FutureCanceled) -> Self {
        PersistenceError::ErrorGeneric("Failed future".to_string())
    }
}

impl From<NoneError> for PersistenceError {
    fn from(_: NoneError) -> Self {
        PersistenceError::ErrorGeneric("Expected Some and got None".to_string())
    }
}

impl From<hcid::HcidError> for PersistenceError {
    fn from(error: hcid::HcidError) -> Self {
        PersistenceError::ErrorGeneric(format!("{:?}", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // a test function that returns our error result
    fn raises_holochain_persistence_error(yes: bool) -> Result<(), PersistenceError> {
        if yes {
            Err(PersistenceError::new("borked"))
        } else {
            Ok(())
        }
    }

    #[test]
    /// test that we can convert an error to a string
    fn to_string() {
        let err = PersistenceError::new("foo");
        assert_eq!("foo", err.to_string());
    }

    #[test]
    /// test that we can convert an error to valid JSON
    fn test_to_json() {
        let err = PersistenceError::new("foo");
        assert_eq!(
            JsonString::from_json("{\"ErrorGeneric\":\"foo\"}"),
            JsonString::from(err),
        );
    }

    #[test]
    /// smoke test new errors
    fn can_instantiate() {
        let err = PersistenceError::new("borked");

        assert_eq!(PersistenceError::ErrorGeneric("borked".to_string()), err);
    }

    #[test]
    /// test errors as a result and destructuring
    fn can_raise_holochain_persistence_error() {
        let err = raises_holochain_persistence_error(true)
            .expect_err("should return an error when yes=true");

        match err {
            PersistenceError::ErrorGeneric(msg) => assert_eq!(msg, "borked"),
            _ => panic!("raises_holochain_persistence_error should return an ErrorGeneric"),
        };
    }

    #[test]
    /// test errors as a returned result
    fn can_return_result() {
        let result = raises_holochain_persistence_error(false);

        assert!(result.is_ok());
    }

    #[test]
    /// show Error implementation for PersistenceError
    fn error_test() {
        for (input, output) in vec![
            (PersistenceError::ErrorGeneric(String::from("foo")), "foo"),
            (
                PersistenceError::SerializationError(String::from("foo")),
                "foo",
            ),
            (PersistenceError::IoError(String::from("foo")), "foo"),
        ] {
            assert_eq!(output, &input.to_string());
        }
    }
}
