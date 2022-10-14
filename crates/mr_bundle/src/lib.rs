//! Library for collecting and packing resources into a bundle with a manifest
//! file which describes those resources.
//!
//! A [`Bundle`](crate::Bundle) contains a [`Manifest`](crate::Manifest) as well as any number of arbitrary,
//! opaque resources in the form of [`ResourceBytes`](crate::ResourceBytes).
//! A Bundle can be serialized and written to a file.
//!
//! A Bundle can also be [packed](Bundle::pack_yaml) and [unpacked](Bundle::unpack_yaml),
//! via the `"packing"` feature.
//! Bundle packing is performed by following the [`Location`](crate::Location)s specified in the
//! Manifest as "Bundled", and pulling them into the Bundle that way.
//! Unpacking is done by specifying a target directory and creating a new file
//! for each resource at a relative path specified by the Manifest.

#![warn(missing_docs)]

mod bundle;
mod encoding;
pub mod error;
mod location;
mod manifest;
mod resource;
pub(crate) mod util;

#[cfg(feature = "packing")]
mod packing;

pub use bundle::{Bundle, RawBundle};
pub use encoding::{decode, encode};
pub use location::Location;
pub use manifest::Manifest;
pub use resource::ResourceBytes;
