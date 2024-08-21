pub mod authority_spec;
pub mod authorized_spec_change;
pub mod change_rule;
pub mod app_binding;
pub mod key_anchor;
pub mod key_meta;
pub mod key_registration;
pub mod keyset_root;

pub use authorized_spec_change::*;
pub use authority_spec::*;
pub use change_rule::*;
pub use app_binding::*;
pub use keyset_root::*;
pub use key_anchor::*;
pub use key_meta::*;
pub use key_registration::*;

use std::collections::BTreeMap;


pub type MetaData = BTreeMap<String, rmpv::Value>;

pub use hdi;