//! # Examples
//!
//! ```
//! use hdi::prelude::*;
//! #[hdi_entry_helper]
//! pub struct Post(pub String);
//! #[hdi_entry_helper]
//! pub struct Msg(pub String);
//!
//! #[hdi_entry_helper]
//! pub struct PrivMsg(pub String);
//!
//! #[hdi_entry_types]
//! #[unit_enum(UnitEntryTypes)]
//! pub enum EntryTypes {
//!     Post(Post),
//!     #[entry_type(required_validations = 5)]
//!     Msg(Msg),
//!     #[entry_type(name = "hidden_msg", required_validations = 5, visibility = "private")]
//!     PrivMsg(PrivMsg),
//! }
//! # fn main() {
//! assert_eq!(__num_entry_types(), 3);
//! # }
//! ```

use self::hdi::prelude::*;
use crate as hdi;

#[hdi_entry_helper]
pub struct Post(pub String);
#[hdi_entry_helper]
pub struct Msg(pub String);

#[hdi_entry_helper]
pub struct PrivMsg(pub String);

#[hdi_entry_types]
#[unit_enum(UnitEntryTypes)]
pub enum EntryTypes {
    Post(Post),
    #[entry_type(required_validations = 5)]
    Msg(Msg),
    #[entry_type(name = "hidden_msg", required_validations = 5, visibility = "private")]
    PrivMsg(PrivMsg),
}
