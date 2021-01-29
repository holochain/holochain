use super::error::MrBundleResult;
use crate::error::{ExplodeError, ExplodeResult};
use std::path::{Path, PathBuf};

pub fn encode<T: serde::ser::Serialize>(data: &T) -> MrBundleResult<Vec<u8>> {
    Ok(rmp_serde::to_vec_named(data)?)
}

pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> MrBundleResult<T> {
    Ok(rmp_serde::from_read_ref(bytes)?)
}

/// Removes a subpath suffix from a path
pub fn prune_path<P: AsRef<Path>>(mut path: PathBuf, subpath: P) -> ExplodeResult<PathBuf> {
    if path.ends_with(&subpath) {
        for _ in subpath.as_ref().components() {
            let _ = path.pop();
        }
        Ok(path)
    } else {
        Err(ExplodeError::ManifestPathSuffixMismatch(
            path,
            subpath.as_ref().to_owned(),
        ))
    }
}
