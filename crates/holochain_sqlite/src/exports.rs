//! A few imports from `rkv`, to avoid consumers needing to import `rkv` explicitly

use crate::table::Table;

/// Simple type alias for re-exporting
pub type SingleTable = Table;
/// Simple type alias for re-exporting
pub type IntegerTable = Table;
/// Simple type alias for re-exporting
pub type MultiTable = Table;

pub use fallible_iterator::FallibleIterator;
