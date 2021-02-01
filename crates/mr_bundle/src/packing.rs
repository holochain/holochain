use super::Bundle;
use crate::{
    error::{MrBundleResult, PackingError, UnpackingError, UnpackingResult},
    util::prune_path,
    Manifest,
};
use std::path::Path;

impl<M: Manifest> Bundle<M> {
    /// Create a directory which contains the manifest as a YAML file,
    /// and each resource written to its own file (as raw bytes)
    /// The paths of the resources are specified by the paths of the bundle,
    /// and the path of the manifest file is specified by the `Manifest::path`
    /// trait method implementation of the `M` type.
    pub async fn unpack_yaml(&self, base_path: &Path, force: bool) -> MrBundleResult<()> {
        self.unpack_yaml_inner(base_path, force)
            .await
            .map_err(Into::into)
    }

    async fn unpack_yaml_inner(&self, base_path: &Path, force: bool) -> UnpackingResult<()> {
        if !force {
            if base_path.exists() {
                return Err(UnpackingError::DirectoryExists(base_path.to_owned()));
            }
        }
        crate::fs(base_path).create_dir_all().await?;
        let base_path = base_path.canonicalize()?;
        crate::fs(&base_path).create_dir_all().await?;
        for (relative_path, resource) in self.bundled_resources() {
            let path = base_path.join(&relative_path);
            let path_clone = path.clone();
            let parent = path_clone
                .parent()
                .clone()
                .ok_or_else(|| UnpackingError::ParentlessPath(path.clone()))?;
            crate::fs(&parent).create_dir_all().await?;
            crate::fs(&path).write(resource).await?;
        }
        let yaml_str = serde_yaml::to_string(self.manifest())?;
        let manifest_path = base_path.join(self.manifest().path());
        crate::fs(&manifest_path).write(yaml_str.as_bytes()).await?;
        Ok(())
    }

    /// Reconstruct a `Bundle<M>` from a previously unpacked directory.
    /// The manifest file itself must be specified, since it may have an arbitrary
    /// path relative to the unpacked directory root.
    pub async fn pack_yaml(manifest_path: &Path) -> MrBundleResult<Self> {
        let manifest_path = manifest_path.canonicalize()?;
        let manifest_yaml = crate::fs(&manifest_path)
            .read_to_string()
            .await
            .map_err(|err| {
                PackingError::BadManifestPath(manifest_path.clone(), err.into_inner())
            })?;
        let manifest: M = serde_yaml::from_str(&manifest_yaml).map_err(UnpackingError::from)?;
        let manifest_relative_path = manifest.path();
        let base_path = prune_path(manifest_path.clone(), &manifest_relative_path)?;
        let resources = futures::future::join_all(manifest.bundled_paths().into_iter().map(
            |relative_path| async {
                let resource_path = base_path.join(&relative_path).canonicalize().expect("TODO");
                crate::fs(&resource_path)
                    .read()
                    .await
                    .map(|resource| (relative_path, resource))
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        Ok(Bundle::new(manifest, resources, base_path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_pruning() {
        let path = PathBuf::from("/a/b/c/d");
        assert_eq!(
            prune_path(path.clone(), "d").unwrap(),
            PathBuf::from("/a/b/c")
        );
        assert_eq!(
            prune_path(path.clone(), "b/c/d").unwrap(),
            PathBuf::from("/a")
        );
        matches::assert_matches!(
            prune_path(path.clone(), "a/c"),
            Err(UnpackingError::ManifestPathSuffixMismatch(abs, rel))
            if abs == path && rel == PathBuf::from("a/c")
        );
    }
}
