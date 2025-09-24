//! Example Link Types
//!
//! # Examples
//!
//! ```
//! use hdi::prelude::*;
//! #[hdk_link_types]
//! pub enum SomeLinkTypes {
//!     SomeLinkType,
//!     SomeOtherLinkType,
//! }
//! assert_eq!(__num_link_types(), 2);
//! ```
use crate::prelude::*;

/// This is an example of declaring your link types.
#[hdk_link_types]
pub enum SomeLinkTypes {
    SomeLinkType,
    SomeOtherLinkType,
}
