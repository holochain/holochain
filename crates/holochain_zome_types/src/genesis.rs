//! Types related to the genesis process whereby a user commits their initial
//! records and validates them to the best of their ability. Full validation
//! may not be possible if network access is required, so they perform a
//! "self-check" (as in "check yourself before you wreck yourself") before
//! joining to ensure that they can catch any problems they can before being
//! subject to the scrutiny of their peers and facing possible rejection.

//! For more details see [`holochain_integrity_types::genesis`].

#[doc(no_inline)]
pub use holochain_integrity_types::genesis;

#[doc(inline)]
pub use holochain_integrity_types::genesis::*;
