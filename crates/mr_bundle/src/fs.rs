use super::Bundle;
use crate::{
    bundle::ResourceMap,
    error::MrBundleResult,
    Manifest, RawBundle,
};
use std::path::Path;

impl<M: Manifest> Bundle<M> {
    /// Create a directory which contains the manifest as a YAML file,
    /// and each resource written to its own file.
    ///
    /// The manifest specifies its own filename with [`Manifest::file_name`]. Resources get a
    /// [ResourceIdentifier] at the time they are packaged, which will be used as the filename
    /// when unpacking. The content of each resource is the same as the original.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub async fn unpack_to_dir(&self, base_path: &Path, force: bool) -> MrBundleResult<()> {
        unpack_to_dir(
            self.manifest(),
            self.get_all_resources(),
            base_path,
            M::file_name().as_ref(),
            force,
        )
        .await
        .map_err(Into::into)
    }

    /// Reconstruct a `Bundle<M>` from a previously unpacked directory.
    /// The manifest file itself must be specified, since it may have an arbitrary
    /// path relative to the unpacked directory root.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub async fn pack_from_manifest_path(manifest_path: &Path) -> MrBundleResult<Self> {
        let manifest_path = dunce::canonicalize(manifest_path)?;
        let manifest_yaml = tokio::fs::read_to_string(&manifest_path).await?;

        let manifest: M = serde_yaml::from_str(&manifest_yaml).map_err(crate::error::UnpackingError::from)?;
        let manifest_relative_path = M::file_name();
        let base_path = crate::util::prune_path(manifest_path.clone(), &manifest_relative_path)?;
        let resources = futures::future::join_all(manifest.resource_ids().into_iter().map(
            |relative_path| async {
                let resource_path = dunce::canonicalize(base_path.join(&relative_path))?;
                tokio::fs::read(&resource_path)
                    .await
                    .map(|resource| (relative_path, resource.into()))
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        Bundle::new(manifest, resources)
    }
}

#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
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
        unpack_to_dir(
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

#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
async fn unpack_to_dir<M: serde::Serialize>(
    manifest: &M,
    resources: &ResourceMap,
    base_path: &Path,
    manifest_path: &Path,
    force: bool,
) -> crate::error::UnpackingResult<()> {
    // If the directory already exists, and we're not forcing, then we can't continue.
    if !force && base_path.exists() {
        return Err(crate::error::UnpackingError::DirectoryExists(base_path.to_owned()));
    }

    // Create the directory to work into.
    tokio::fs::create_dir_all(&base_path).await?;

    for (relative_path, resource) in resources {
        let path = base_path.join(relative_path);
        let path_clone = path.clone();
        let parent = path_clone
            .parent()
            .ok_or_else(|| crate::error::UnpackingError::ParentlessPath(path.clone()))?;
        tokio::fs::create_dir_all(&parent).await?;
        tokio::fs::write(&path, resource.inner()).await?;
    }
    let yaml_str = serde_yaml::to_string(manifest)?;
    let manifest_path = base_path.join(manifest_path);
    tokio::fs::write(&manifest_path, yaml_str.as_bytes()).await?;
    Ok(())
}

#[cfg(all(test, feature = "fs"))]
mod tests {
    use crate::error::UnpackingError;
    use crate::util::prune_path;
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
