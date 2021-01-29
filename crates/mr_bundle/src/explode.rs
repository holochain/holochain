use super::Bundle;
use crate::{
    error::{ExplodeError, ExplodeResult, ImplodeError, MrBundleResult},
    Manifest,
};
use std::path::{Path, PathBuf};

impl<M: Manifest> Bundle<M> {
    /// Create a directory which contains the manifest as a YAML file,
    /// and each resource written to its own file (as raw bytes)
    /// The paths of the resources are specified by the paths of the bundle,
    /// and the path of the manifest file is specified by the `Manifest::path`
    /// trait method implementation of the `M` type.
    pub async fn explode_yaml(&self, base_path: &Path) -> ExplodeResult<()> {
        crate::fs::create_dir_all(base_path).await?;
        let base_path = base_path.canonicalize()?;
        crate::fs::create_dir_all(&base_path).await?;
        for (relative_path, resource) in self.bundled_resources() {
            let path = base_path.join(&relative_path);
            let path_clone = path.clone();
            let parent = path_clone
                .parent()
                .clone()
                .ok_or_else(|| ExplodeError::ParentlessPath(path.clone()))?;
            crate::fs::create_dir_all(&parent).await?;
            crate::fs::write(&path, resource).await?;
        }
        let yaml_str = serde_yaml::to_string(self.manifest())?;
        let manifest_path = base_path.join(self.manifest().path());
        crate::fs::write(&manifest_path, yaml_str.as_bytes()).await?;
        Ok(())
    }

    /// Reconstruct a `Bundle<M>` from a previously exploded directory.
    /// The manifest file itself must be specified, since it may have an arbitrary
    /// path relative to the exploded directory root.
    pub async fn implode_yaml(manifest_path: &Path) -> MrBundleResult<Self> {
        let manifest_path = manifest_path.canonicalize()?;
        let manifest_yaml = crate::fs::read_to_string(&manifest_path)
            .await
            .map_err(|err| ImplodeError::BadManifestPath(manifest_path.clone(), err))?;
        let manifest: M = serde_yaml::from_str(&manifest_yaml).map_err(ExplodeError::from)?;
        let manifest_relative_path = manifest.path();
        let base_path =
            prune_path(manifest_path.clone(), &manifest_relative_path).ok_or_else(|| {
                ExplodeError::ManifestPathSuffixMismatch(
                    manifest_path,
                    manifest_relative_path.clone(),
                )
            })?;
        let resources = futures::future::join_all(manifest.bundled_paths().into_iter().map(
            |relative_path| async {
                let resource_path = base_path.join(&relative_path);
                crate::fs::read(&resource_path)
                    .await
                    .map(|resource| (relative_path, resource))
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        Ok(Bundle::new(manifest, resources)?)
    }
}

/// Removes a subpath suffix from a path
fn prune_path<P: AsRef<Path>>(mut path: PathBuf, subpath: P) -> Option<PathBuf> {
    if path.ends_with(&subpath) {
        for _ in subpath.as_ref().components() {
            let _ = path.pop();
        }
        Some(path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pruning() {
        let path = PathBuf::from("/a/b/c/d");
        assert_eq!(prune_path(path.clone(), "d"), Some(PathBuf::from("/a/b/c")));
        assert_eq!(prune_path(path.clone(), "b/c/d"), Some(PathBuf::from("/a")));
        assert_eq!(prune_path(path.clone(), "a/c"), None);
    }
}
