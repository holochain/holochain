// The following are a convenience used in various downstream crates.
// It is included here because it is the most upstream crate in the kitsune
// crate set which makes use of it. It could make more sense to put this
// into some kind of utility crate, but that doesn't exist yet.

/// Add the Arbitrary constraint if feature "fuzzing" is enabled.
/// Otherwise, no constraint added
// #[cfg(feature = "fuzzing")]
// pub trait ArbitraryFuzzing: proptest::arbitrary::Arbitrary {}
// #[cfg(feature = "fuzzing")]
// impl<T> ArbitraryFuzzing for T where T: proptest::arbitrary::Arbitrary {}

/// Add the Arbitrary constraint if feature "fuzzing" is enabled.
/// Otherwise, no constraint added
// #[cfg(not(feature = "fuzzing"))]
pub trait ArbitraryFuzzing {}
// #[cfg(not(feature = "fuzzing"))]
impl<T> ArbitraryFuzzing for T {}
