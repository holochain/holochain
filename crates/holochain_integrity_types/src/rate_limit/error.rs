use kitsune_p2p_timestamp::{Timestamp, TimestampError};

use crate::RateBucketId;

/// Errors involving app entry creation
#[derive(Debug, Clone, PartialEq)]
pub enum RateBucketError {
    /// The bucket index was not defined
    BucketIdMissing(RateBucketId),
    /// The bucket has overflowed its capacity
    BucketOverflow,
    /// A bucket attempted to process an item with an earlier timestamp than the last
    NonMonotonicTimestamp(Timestamp, Timestamp),
    /// Other Timestamp error
    TimestampError(TimestampError),
}

impl std::error::Error for RateBucketError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RateBucketError::BucketIdMissing(_) => None,
            RateBucketError::BucketOverflow => None,
            RateBucketError::NonMonotonicTimestamp(_, _) => None,
            RateBucketError::TimestampError(e) => e.source(),
        }
    }
}

impl From<TimestampError> for RateBucketError {
    fn from(e: TimestampError) -> Self {
        Self::TimestampError(e)
    }
}

impl core::fmt::Display for RateBucketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RateBucketError::BucketIdMissing(id) => write!(
                f,
                "There is no bucket defined at index {0}.",
                id,
            ),
            RateBucketError::BucketOverflow => write!(
                f,
                "A bucket overflowed. Rate limit exceeded.",
            ),
            RateBucketError::NonMonotonicTimestamp(t1, t2) => write!(
                f,
                "Tried to process a timestamp which was behind the previous one. previous={:?}, current={:?}",
                t1, t2,
            ),
            RateBucketError::TimestampError(e) => e.fmt(f)
        }
    }
}

/// Result type for RateBucketError
pub type RateBucketResult<T> = Result<T, RateBucketError>;
