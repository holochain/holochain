//! # Examples
//!
//! ```
//! use hdi::prelude::*;
//! #[hdk_entry_helper]
//! pub struct Post(pub String);
//! #[hdk_entry_helper]
//! pub struct Msg(pub String);
//!
//! #[hdk_entry_helper]
//! pub struct PrivMsg(pub String);
//!
//! #[hdk_entry_defs]
//! #[unit_enum(UnitEntryTypes)]
//! pub enum EntryTypes {
//!     Post(Post),
//!     #[entry_def(required_validations = 5)]
//!     Msg(Msg),
//!     #[entry_def(name = "hidden_msg", required_validations = 5, visibility = "private")]
//!     PrivMsg(PrivMsg),
//! }
//! # fn main() {
//! assert_eq!(__num_entry_types(), 3);
//! # }
//! ```

use self::hdi::prelude::*;
use crate as hdi;

#[hdk_entry_helper]
pub struct Post(pub String);
#[hdk_entry_helper]
pub struct Msg(pub String);

#[hdk_entry_helper]
pub struct PrivMsg(pub String);

#[hdk_entry_defs]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    Post(Post),
    #[entry_def(required_validations = 5)]
    Msg(Msg),
    #[entry_def(name = "hidden_msg", required_validations = 5, visibility = "private")]
    PrivMsg(PrivMsg),
}
