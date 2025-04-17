use super::{Bundle, ResourceIdentifier};
use crate::{
    bundle::ResourceMap,
    error::MrBundleResult,
    Manifest, RawBundle,
};
use std::path::Path;

/// A recommended conversion from a path to a resource identifier.
///
/// Calling this function with its own output will produce the same result.
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub fn resource_id_for_path(path: impl AsRef<Path>) -> Option<ResourceIdentifier> {
    let path = path.as_ref();
    if path.parent().is_some() {
        path.file_name().and_then(|n| n.to_str()).map(|s| s.to_string())
    } else {
        path.to_str().map(|s| s.to_string())
    }
}

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
        .await?;

        Ok(())
    }

    /// Reconstruct a `Bundle<M>` from a previously unpacked directory.
    /// The manifest file itself must be specified, since it may have an arbitrary
    /// path relative to the unpacked directory root.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    pub async fn pack_from_manifest_path(manifest_path: impl AsRef<Path>) -> MrBundleResult<Self> {
        let manifest_path = dunce::canonicalize(manifest_path)?;
        let manifest_yaml = tokio::fs::read_to_string(&manifest_path).await?;
        let mut manifest: M = serde_yaml::from_str(&manifest_yaml).map_err(crate::error::UnpackingError::from)?;

        let manifest_dir = manifest_path.parent().ok_or_else(|| {
            crate::error::UnpackingError::ParentlessPath(manifest_path.to_path_buf())
        })?;
        let resources = futures::future::join_all(manifest.generate_resource_ids().into_iter().map(
            |(resource_id, relative_path)| async {
                let resource_path = dunce::canonicalize(manifest_dir.join(relative_path))?;
                tokio::fs::read(&resource_path)
                    .await
                    .map(|resource| (resource_id, resource.into()))
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
    pub async fn unpack_to_dir(
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

    for (resource_id, resource) in resources {
        let path = base_path.join(resource_id);
        let path_clone = path.clone();
        let parent = path_clone
            .parent()
            .ok_or_else(|| crate::error::UnpackingError::ParentlessPath(path.clone()))?;
        tokio::fs::create_dir_all(&parent).await?;
        tokio::fs::write(&path, resource).await?;
    }
    let yaml_str = serde_yaml::to_string(manifest)?;
    let manifest_path = base_path.join(manifest_path);
    tokio::fs::write(&manifest_path, yaml_str.as_bytes()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_path_to_resource_id() {
        // A simple file name should be unchanged
        assert_eq!("hello.txt", resource_id_for_path("hello.txt").unwrap());

        // Absolute path to file should become the file name
        assert_eq!("hello.txt", resource_id_for_path("/dir/hello.txt").unwrap());

        // A relative path to a file should become the file name
        assert_eq!("hello.txt", resource_id_for_path("../../dir/hello.txt").unwrap());

        // Relative file path should become the file name
        assert_eq!("hello.txt", resource_id_for_path("./hello.txt").unwrap());
    }
}
