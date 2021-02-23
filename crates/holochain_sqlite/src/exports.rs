//! A few imports from `rkv`, to avoid consumers needing to import `rkv` explicitly

use crate::{db::Table, prelude::IntKey};

/// Simple type alias for re-exporting
pub type SingleStore = Table;
/// Simple type alias for re-exporting
pub type IntegerStore = Table;
/// Simple type alias for re-exporting
pub type MultiStore = Table;

pub use fallible_iterator::FallibleIterator;
