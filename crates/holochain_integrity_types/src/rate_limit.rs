//! Rate limiting data types

use holochain_serialized_bytes::prelude::*;

use crate::{Create, CreateLink, Delete, Entry, Update, MAX_ENTRY_SIZE};

mod bucket;
pub use bucket::*;
mod error;
pub use error::*;

/// Input to the `weigh` callback. Includes an "unweighed" header, and Entry
/// if applicable.
#[derive(Clone, PartialEq, Serialize, Deserialize, SerializedBytes, Debug)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum WeighInput {
    /// A Link to be weighed
    Link(CreateLink<()>),
    /// A new Entry to be weighed
    Create(Create<()>, Entry),
    /// An updated Entry to be weighed
    Update(Update<()>, Entry),
    /// An Entry deletion to be weighed
    Delete(Delete<()>),
}

/// A bucket ID, for rate limiting
pub type RateBucketId = u8;

/// The weight of this header, for rate limiting
pub type RateUnits = u8;

/// The normalized total size of this header, for rate limiting
pub type RateBytes = u8;

/// The amount that a bucket is "filled"
pub type RateBucketCapacity = u32;

/// Combination of two rate limiting data types, for convenience
#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
    Hash,
    PartialOrd,
    Ord,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[allow(missing_docs)]
pub struct RateWeight {
    pub bucket_id: RateBucketId,
    pub units: RateUnits,
}

impl Default for RateWeight {
    fn default() -> Self {
        Self {
            bucket_id: 255,
            units: 0,
        }
    }
}

/// Combination of the three main rate limiting data types, for convenience
#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    SerializedBytes,
    Hash,
    PartialOrd,
    Ord,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[allow(missing_docs)]
pub struct EntryRateWeight {
    pub bucket_id: RateBucketId,
    pub units: RateUnits,
    pub rate_bytes: RateBytes,
}

impl Default for EntryRateWeight {
    fn default() -> Self {
        Self {
            bucket_id: 255,
            units: 0,
            rate_bytes: 0,
        }
    }
}

impl From<EntryRateWeight> for RateWeight {
    fn from(w: EntryRateWeight) -> Self {
        Self {
            bucket_id: w.bucket_id,
            units: w.units,
        }
    }
}

impl EntryRateWeight {
    /// Add the rate_bytes field to a RateWeight to produce an EntryRateWeight
    pub fn from_weight_and_size(w: RateWeight, size: usize) -> Self {
        let rate_bytes = (255 * size / MAX_ENTRY_SIZE).min(255) as u8;
        Self {
            bucket_id: w.bucket_id,
            units: w.units,
            rate_bytes,
        }
    }
}
