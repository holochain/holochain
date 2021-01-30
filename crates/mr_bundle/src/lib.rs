mod bundle;
pub mod error;
mod fs;
pub mod io_error;
mod location;
mod manifest;
mod resource;
pub(crate) mod util;

#[cfg(feature = "packing")]
mod packing;

pub use bundle::Bundle;
pub use fs::fs;
pub use location::Location;
pub use manifest::Manifest;
pub use resource::ResourceBytes;
pub use util::{decode, encode};
