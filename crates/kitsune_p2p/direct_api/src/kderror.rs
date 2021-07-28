//! kdirect kderror type

use crate::*;

/// Kitsune Direct Error Type
#[derive(Debug, Clone)]
pub enum KdError {
    /// Temp error type for internal logic.
    Unit,

    /// Unspecified error.
    Other(Arc<dyn std::error::Error + Send + Sync>),
}

impl std::error::Error for KdError {}

impl KdError {
    /// promote a custom error type to a KdError
    pub fn other(e: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self::Other(e.into().into())
    }
}

impl std::fmt::Display for KdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<String> for KdError {
    fn from(s: String) -> Self {
        #[derive(Debug)]
        struct OtherError(pub String);
        impl std::error::Error for OtherError {}
        impl std::fmt::Display for OtherError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{:?}", self)
            }
        }
        KdError::other(OtherError(s))
    }
}

impl From<&String> for KdError {
    fn from(s: &String) -> Self {
        s.to_string().into()
    }
}

impl From<&str> for KdError {
    fn from(s: &str) -> Self {
        s.to_string().into()
    }
}

impl From<KdError> for () {
    fn from(_: KdError) {}
}

impl From<()> for KdError {
    fn from(_: ()) -> Self {
        KdError::Unit
    }
}

/// Kitsune Direct Result Type
pub type KdResult<T> = std::result::Result<T, KdError>;
