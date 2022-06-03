//! Types related to rate limiting

/// A list of current bucket levels. The buckets are indexable by bucket id.
/// Attempts to index a bucket out of range simply returns a value of `0`.
///
/// This type is used in the holochain database to store the state of buckets
/// as at a particular Header. The out-of-range indexability allows for a sparser
/// representation when buckets are empty. In particular, it allows an empty
/// Vec to be used when all buckets are empty.
#[derive(Default, Clone)]
pub struct RateBucketLevels(Vec<RateBucketCapacity>);

pub use holochain_zome_types::rate_limit::*;
use itertools::Itertools;

impl RateBucketLevels {
    /// The the level of the bucket an index. If the index is beyond the range
    /// of known values, 0 is returned.
    pub fn get(&self, index: u8) -> RateBucketCapacity {
        self.0.get(index as usize).copied().unwrap_or_default()
    }

    /// Set a bucket value, resizing the vec if necessary.
    pub fn set(&mut self, index: u8, level: RateBucketCapacity) {
        let index = index as usize;
        if self.0.len() < index + 1 {
            self.0.resize(index + 1, 0);
        }
        self.0[index] = level;
        if self.0.len() >= index + 1 {
            let reduction = self
                .0
                .iter()
                .rev()
                .find_position(|i| **i > 0)
                .map(|(i, _)| i)
                // If all values are 0, resize the vec to 0 length
                .unwrap_or(self.0.len());
            self.0.resize(self.0.len() - reduction, 0);
        }
    }

    /// Reconstruct from a flattened byte array using big-endian values
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        const SZ: usize = 4;
        // if this assertion fails due to RateBucketCapacity using more or less bytes,
        // then databases need to be migrated to include a recomputation of all
        // bucket states in the database.
        debug_assert_eq!(
            SZ,
            std::mem::size_of::<RateBucketCapacity>(),
            "RateBucketCapacity is expected to be a 4 byte value"
        );
        if bytes.len() % SZ == 0 {
            Some(RateBucketLevels(
                bytes
                    .chunks(SZ)
                    .map(|bytes| {
                        let four = <[u8; SZ]>::try_from(bytes).unwrap();
                        RateBucketCapacity::from_be_bytes(four)
                    })
                    .collect(),
            ))
        } else {
            None
        }
    }

    /// Serialize to a flat byte array using big-endian values
    pub fn to_bytes(&self) -> Vec<u8> {
        // if this assertion fails due to RateBucketCapacity using more or less bytes,
        // then databases need to be migrated to include a recomputation of all
        // bucket states in the database.
        debug_assert_eq!(
            4,
            std::mem::size_of::<RateBucketCapacity>(),
            "RateBucketCapacity is expected to be a 4 byte value"
        );
        self.0
            .iter()
            .flat_map(|level| level.to_be_bytes())
            .collect()
    }
}

impl rusqlite::types::FromSql for RateBucketLevels {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            // NB: if you have a NULLable Timestamp field in a DB, use `Option<Timestamp>`.
            //     otherwise, you'll get an InvalidType error, because we don't handle null
            //     values here.
            rusqlite::types::ValueRef::Blob(b) => Self::from_bytes(b).ok_or_else(|| {
                rusqlite::types::FromSqlError::Other(
                    format!(
                    "Invalid RateBucketLevels value, length must be a multiple of 4. Blob: {:?}",
                    b
                )
                    .into(),
                )
            }),
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}

impl rusqlite::ToSql for RateBucketLevels {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput> {
        Ok(rusqlite::types::ToSqlOutput::Owned(self.to_bytes().into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_resizing() {
        let mut buckets = RateBucketLevels::default();
        assert_eq!(buckets.0.len(), 0);
        assert_eq!(buckets.get(1), 0);
        assert_eq!(buckets.get(3), 0);
        assert_eq!(buckets.get(5), 0);
        buckets.set(3, 3);
        assert_eq!(buckets.0.len(), 4);
        assert_eq!(buckets.get(1), 0);
        assert_eq!(buckets.get(2), 0);
        assert_eq!(buckets.get(3), 3);
        buckets.set(1, 1);
        assert_eq!(buckets.0.len(), 4);
        assert_eq!(buckets.get(1), 1);
        assert_eq!(buckets.get(2), 0);
        assert_eq!(buckets.get(3), 3);
        buckets.set(3, 0);
        assert_eq!(buckets.0.len(), 2);
        assert_eq!(buckets.get(1), 1);
        assert_eq!(buckets.get(2), 0);
        assert_eq!(buckets.get(3), 0);
        buckets.set(5, 5);
        assert_eq!(buckets.0.len(), 6);
        assert_eq!(buckets.get(1), 1);
        assert_eq!(buckets.get(3), 0);
        assert_eq!(buckets.get(5), 5);
        buckets.set(1, 0);
        buckets.set(5, 0);
        assert_eq!(buckets.0.len(), 0);
    }
}
