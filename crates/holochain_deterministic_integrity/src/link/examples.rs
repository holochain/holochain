//! Example Link Types
//!
//! # Examples
//!
//! ```
//! use holochain_deterministic_integrity::prelude::*;
//! #[hdk_link_types]
//! pub enum SomeLinkTypes {
//!     SomeLinkType,
//!     SomeOtherLinkType,
//! }
//! assert_eq!(__num_link_types(), 2);
//! ```
use crate::prelude::*;

#[hdk_link_types]
/// This is an example of declaring your link types.
pub enum SomeLinkTypes {
    SomeLinkType,
    SomeOtherLinkType,
}
