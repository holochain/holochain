//! A few imports from `rkv`, to avoid consumers needing to import `rkv` explicitly

/// Simple type alias for re-exporting
pub type SingleStore = rkv::SingleStore;
/// Simple type alias for re-exporting
pub type IntegerStore = rkv::IntegerStore<u32>;
/// Simple type alias for re-exporting
pub type MultiStore = rkv::MultiStore;
