//! Paths for the keystore.

use std::path::PathBuf;

/// Subdirectory of the data directory where the conductor stores its
/// keystore. Keep the path short so that when it's used in CI the path doesn't
/// get too long to be used as a domain socket
pub const KEYSTORE_DIRECTORY: &str = "ks";

/// Newtype to make sure we never accidentaly use or not use the keystore path.
/// Intentionally has no default value.
#[derive(
    shrinkwraprs::Shrinkwrap,
    derive_more::From,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    Clone,
)]
pub struct KeystorePath(PathBuf);
