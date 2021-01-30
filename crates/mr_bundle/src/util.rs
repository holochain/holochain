use super::error::MrBundleResult;
use std::path::{Path, PathBuf};

pub fn encode<T: serde::ser::Serialize>(data: &T) -> MrBundleResult<Vec<u8>> {
    Ok(rmp_serde::to_vec_named(data)?)
}

pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> MrBundleResult<T> {
    Ok(rmp_serde::from_read_ref(bytes)?)
}

#[cfg(feature = "packing")]
use crate::error::{UnpackingError, UnpackingResult};

/// Removes a subpath suffix from a path
#[cfg(feature = "packing")]
pub fn prune_path<P: AsRef<Path>>(mut path: PathBuf, subpath: P) -> UnpackingResult<PathBuf> {
    if path.ends_with(&subpath) {
        for _ in subpath.as_ref().components() {
            let _ = path.pop();
        }
        Ok(path)
    } else {
        Err(UnpackingError::ManifestPathSuffixMismatch(
            path,
            subpath.as_ref().to_owned(),
        ))
    }
}
