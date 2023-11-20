use kitsune_p2p_types::KOpHash;
use std::hash::Hash;

/// The granularity once we're > i16::MAX
const GRAN: usize = 4096;

// TODO: try a u32 -> u16 mapping based on phi, the golden ratio, which keeps the expected error constant
//       at any scale. These are some initial calculations to try.
// const G: f64 = 1690.0; // x / (ln_phi 128000000)
// const LOW: f64 = 15.0 * G;
// const THRESH: u16 = 30000;

/// Roughly track an approximate integer value.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub struct RoughInt(i16);

impl std::fmt::Debug for RoughInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

impl RoughInt {
    /// Maximum value representable by RoughInt, currently 134_213_632
    pub const MAX: usize = i16::MAX as usize * GRAN;

    /// Get the full value from the rough int
    pub fn get(&self) -> usize {
        if self.0 > 0 {
            // positive is exact value
            self.0 as usize
        } else {
            // negative is in chunks of size GRAN
            (-self.0) as usize * GRAN
        }
    }

    /// Set the rough int from the full value
    pub fn set(&mut self, v: usize) -> Self {
        if v <= i16::MAX as usize {
            // if we're under i16::MAX, we can store the exact value
            self.0 = v as i16
        } else {
            // otherwise, divide by GRAN and store as negative
            self.0 = -(std::cmp::min(i16::MAX as usize, v / GRAN) as i16);
        }
        *self
    }
}

impl From<usize> for RoughInt {
    fn from(v: usize) -> Self {
        Self::default().set(v)
    }
}

/// An op hash combined with an approximate size of the op
pub type OpHashSized = RoughSized<KOpHash>;

/// Some data which has a RoughInt assigned for its size
#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    derive_more::From,
    derive_more::Into,
    derive_more::Constructor,
    derive_more::Deref,
)]
#[cfg_attr(feature = "fuzzing", derive(proptest_derive::Arbitrary))]
pub struct RoughSized<T> {
    /// The data to be sized
    #[deref]
    data: T,
    /// The approximate size of the hash.
    // TODO: remove the option, which will require adding sizes for Recent gossip as well
    size: Option<RoughInt>,
}

impl<T> RoughSized<T> {
    /// Break into constituent parts
    pub fn into_inner(self) -> (T, Option<RoughInt>) {
        (self.data, self.size)
    }

    /// Accessor
    pub fn data_ref(&self) -> &T {
        &self.data
    }

    /// Accessor
    pub fn maybe_size(&self) -> Option<RoughInt> {
        self.size
    }

    /// Accessor
    pub fn size(&self) -> RoughInt {
        self.size.unwrap_or_default()
    }
}

impl<T: Clone> RoughSized<T> {
    /// Accessor
    pub fn data(&self) -> T {
        self.data.clone()
    }
}

impl<T: Hash> Hash for RoughSized<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // size is omitted from the hash
        self.data.hash(state);
    }
}

impl<T: PartialEq> PartialEq for RoughSized<T> {
    fn eq(&self, other: &Self) -> bool {
        // size is omitted from the equality
        self.data == other.data
    }
}

impl<T: Eq> Eq for RoughSized<T> {}

#[cfg(feature = "fuzzing")]
impl<'a, T: arbitrary::Arbitrary<'a>> arbitrary::Arbitrary<'a> for RoughSized<T> {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            data: arbitrary::Arbitrary::arbitrary(u)?,
            size: arbitrary::Arbitrary::arbitrary(u)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The percent error is always less than 12%, and the max error occurs right around
    /// i16::MAX, tapering off as values grow larger
    #[test]
    fn error_upper_bound() {
        let m16 = i16::MAX as usize;
        for m in [
            m16,
            m16 + 2,
            m16 + GRAN - 1,
            m16 + GRAN * 10 - 1,
            RoughInt::MAX - 1,
            RoughInt::MAX,
        ] {
            let r = RoughInt::from(m).get();
            let error = r.abs_diff(m) as f64 / m as f64;
            dbg!(r, m, error);
            assert!(error < 0.13);
        }
    }

    proptest! {
        #[test]
        fn roughint_roundtrip(v: usize) {
            let r = RoughInt::from(v);
            let v = r.get();
            assert_eq!(r, RoughInt::from(v));
        }

        #[test]
        fn roughint_always_underestimates(actual: usize) {
            let rough = RoughInt::from(actual);
            assert!(rough.get() <= actual);
        }

        /// Test that the sum of roughints is less than the max error for a single roughint,
        /// even in the most problematic range
        #[test]
        fn roughint_sum_error_problematic_range(
            real in proptest::collection::vec(i16::MAX as usize..i16::MAX as usize + GRAN, 1..10)
        ) {
            let rough = real.iter().copied().map(RoughInt::from).map(|r| r.get());
            let both: Vec<(usize, usize)> = real.iter().copied().zip(rough).collect();
            let real_sum: usize = both.iter().map(|(r, _)| r).sum();
            let rough_sum: usize = both.iter().map(|(_, r)| r).sum();

            if real_sum == 0 || rough_sum == 0 {
                unreachable!("zero sum");
            }

            let error = (rough_sum.abs_diff(real_sum)) as f64 / real_sum as f64;
            dbg!(error);
            assert!(error <= 0.13);
        }

        /// Test that the sum of roughints is less than the max error for a single roughint,
        /// across the range of all possible values
        fn roughint_sum_error_full_range(
            real in proptest::collection::vec(1..RoughInt::MAX, 1..10)
        ) {
            let rough = real.iter().copied().map(RoughInt::from).map(|r| r.get());
            let both: Vec<(usize, usize)> = real.iter().copied().zip(rough).collect();
            let real_sum: usize = both.iter().map(|(r, _)| r).sum();
            let rough_sum: usize = both.iter().map(|(_, r)| r).sum();

            if real_sum == 0 || rough_sum == 0 {
                unreachable!("zero sum");
            }

            let error = (rough_sum.abs_diff(real_sum)) as f64 / real_sum as f64;
            dbg!(error);
            assert!(error <= 0.13);
        }
    }
}
