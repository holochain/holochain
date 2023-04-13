use super::Bundle;
use crate::{
    bundle::ResourceMap,
    error::{MrBundleResult, PackingError, UnpackingError, UnpackingResult},
    util::prune_path,
    Manifest, RawBundle,
};
use holochain_util::ffs;
use std::path::Path;

impl<M: Manifest> Bundle<M> {
    /// Create a directory which contains the manifest as a YAML file,
    /// and each resource written to its own file (as raw bytes)
    /// The paths of the resources are specified by the paths of the bundle,
    /// and the path of the manifest file is specified by the `Manifest::path`
    /// trait method implementation of the `M` type.
    pub async fn unpack_yaml(&self, base_path: &Path, force: bool) -> MrBundleResult<()> {
        unpack_yaml(
            self.manifest(),
            self.bundled_resources(),
            base_path,
            M::path().as_ref(),
            force,
        )
        .await
        .map_err(Into::into)
    }

    /// Reconstruct a `Bundle<M>` from a previously unpacked directory.
    /// The manifest file itself must be specified, since it may have an arbitrary
    /// path relative to the unpacked directory root.
    pub async fn pack_yaml(manifest_path: &Path) -> MrBundleResult<Self> {
        let manifest_path = ffs::canonicalize(manifest_path).await?;
        let manifest_yaml = ffs::read_to_string(&manifest_path).await.map_err(|err| {
            PackingError::BadManifestPath(manifest_path.clone(), err.into_inner())
        })?;
        let manifest: M = serde_yaml::from_str(&manifest_yaml).map_err(UnpackingError::from)?;
        let manifest_relative_path = M::path();
        let base_path = prune_path(manifest_path.clone(), &manifest_relative_path)?;
        let resources = futures::future::join_all(manifest.bundled_paths().into_iter().map(
            |relative_path| async {
                let resource_path = ffs::canonicalize(base_path.join(&relative_path)).await?;
                ffs::read(&resource_path)
                    .await
                    .map(|resource| (relative_path, resource.into()))
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        Bundle::new(manifest, resources, base_path)
    }
}

impl<M: serde::Serialize> RawBundle<M> {
    /// Create a directory which contains the manifest as a YAML file,
    /// and each resource written to its own file (as raw bytes)
    /// The paths of the resources are specified by the paths of the bundle,
    /// and the path of the manifest file is specified by the manifest_path parameter.
    pub async fn unpack_yaml(
        &self,
        base_path: &Path,
        manifest_path: &Path,
        force: bool,
    ) -> MrBundleResult<()> {
        unpack_yaml(
            &self.manifest,
            &self.resources,
            base_path,
            manifest_path,
            force,
        )
        .await
        .map_err(Into::into)
    }
}

async fn unpack_yaml<M: serde::Serialize>(
    manifest: &M,
    resources: &ResourceMap,
    base_path: &Path,
    manifest_path: &Path,
    force: bool,
) -> UnpackingResult<()> {
    if !force && base_path.exists() {
        return Err(UnpackingError::DirectoryExists(base_path.to_owned()));
    }
    ffs::create_dir_all(&base_path).await?;
    for (relative_path, resource) in resources {
        let path = base_path.join(relative_path);
        let path_clone = path.clone();
        let parent = path_clone
            .parent()
            .ok_or_else(|| UnpackingError::ParentlessPath(path.clone()))?;
        ffs::create_dir_all(&parent).await?;
        ffs::write(&path, resource).await?;
    }
    let yaml_str = serde_yaml::to_string(manifest)?;
    let manifest_path = base_path.join(manifest_path);
    ffs::write(&manifest_path, yaml_str.as_bytes()).await?;
    Ok(())
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