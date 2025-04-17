#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
use std::path::{Path, PathBuf};

#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
use crate::error::{UnpackingError, UnpackingResult};

/// Removes a subpath suffix from a path
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
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
