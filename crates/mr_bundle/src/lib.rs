//! Library for collecting and fs resources into a bundle with a manifest
//! file which describes those resources.
//!
//! A [`Bundle`] contains a [`Manifest`] as well as any number of arbitrary,
//! opaque resources in the form of [`ResourceBytes`].
//! A Bundle can be serialized and written to a file.
//!
//! A Bundle can also be [packed](Bundle::pack_from_manifest_path) and [unpacked](Bundle::unpack_to_dir),
//! via the `"fs"` feature.
//! Bundle fs is performed by following the [`Location`]s specified in the
//! Manifest as "Bundled", and pulling them into the Bundle that way.
//! Unpacking is done by specifying a target directory and creating a new file
//! for each resource at a relative path specified by the Manifest.

#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod bundle;
mod encoding;
pub mod error;
mod manifest;
mod fs;

pub use bundle::{raw::RawBundle, resource::ResourceBytes, Bundle, ResourceMap};
pub use encoding::{pack, unpack};
pub use manifest::{Manifest, ResourceIdentifier};

#[cfg(feature = "fs")]
pub use fs::resource_id_for_path;
