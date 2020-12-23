//! A few imports from `rkv`, to avoid consumers needing to import `rkv` explicitly

use crate::prelude::IntKey;

/// Simple type alias for re-exporting
pub type SingleStore = rkv::SingleStore;
/// Simple type alias for re-exporting
pub type IntegerStore = rkv::IntegerStore<IntKey>;
/// Simple type alias for re-exporting
pub type MultiStore = rkv::MultiStore;

pub use fallible_iterator::FallibleIterator;
