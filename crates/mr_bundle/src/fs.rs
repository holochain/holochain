use super::{Bundle, ResourceIdentifier};
use crate::error::MrBundleError;
use crate::{error::MrBundleResult, Manifest};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::path::Path;

/// A recommended conversion from a path to a resource identifier.
///
/// Calling this function with its own output will produce the same result.
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub fn resource_id_for_path(path: impl AsRef<Path>) -> Option<ResourceIdentifier> {
    let path = path.as_ref();
    if path.parent().is_some() {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    } else {
        path.to_str().map(|s| s.to_string())
    }
}

/// A bundler that uses the filesystem to store resources.
///
/// The bundler builds on the [`Manifest`] and [`Bundle`] types and adds file system logic to
/// provide the ability to read and write bundles to the filesystem.
#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
pub struct FileSystemBundler;

#[cfg(feature = "fs")]
#[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
impl FileSystemBundler {
    /// Create a bundle from a manifest file.
    ///
    /// The provided `manifest_path` is expected to be a path to a manifest file. The file is
    /// expected to be a YAML file that can be deserialized into the [`Manifest`] type.
    ///
    /// The resources referenced by the manifest will be loaded from the file system, using
    /// relative paths from the manifest file.
    ///
    /// The resulting [`Bundle`] will contain the manifest and its resources.
    pub async fn bundle<M: Manifest>(manifest_path: impl AsRef<Path>) -> MrBundleResult<Bundle<M>> {
        let manifest_path = dunce::canonicalize(manifest_path).map_err(|e| {
            MrBundleError::IoError("Failed to canonicalize manifest path".to_string(), e)
        })?;
        let manifest_yaml = tokio::fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| {
                MrBundleError::IoError(
                    format!("Failed to read manifest file: {:?}", manifest_path),
                    e,
                )
            })?;
        let mut manifest: M = serde_yaml::from_str(&manifest_yaml)?;

        println!("Manifest: {:?}", manifest);

        let manifest_dir = manifest_path
            .parent()
            .ok_or_else(|| MrBundleError::ParentlessPath(manifest_path.to_path_buf()))?;
        let resources =
            futures::future::join_all(manifest.generate_resource_ids().into_iter().map(
                |(resource_id, relative_path)| async move {
                    println!("Got relative path: {:?}", relative_path);
                    let resource_path = manifest_dir.join(&relative_path);
                    let resource_path = dunce::canonicalize(&resource_path).map_err(|e| {
                        MrBundleError::IoError(
                            format!(
                                "Failed to canonicalize resource path: {}",
                                resource_path.display()
                            ),
                            e,
                        )
                    })?;
                    tokio::fs::read(&resource_path)
                        .await
                        .map(|resource| (resource_id, resource.into()))
                        .map_err(|e| {
                            MrBundleError::IoError(
                                format!("Failed to read resource at path: {:?}", resource_path),
                                e,
                            )
                        })
                },
            ))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        Bundle::new(manifest, resources)
    }

    /// A convenience function that creates a bundle and writes it to the filesystem.
    ///
    /// Uses [`bundle`](FileSystemBundler::bundle) to create the bundle and then writes it to the
    /// provided `bundle_path`.
    pub async fn bundle_to<M: Manifest>(
        manifest_path: impl AsRef<Path>,
        bundle_path: impl AsRef<Path>,
    ) -> MrBundleResult<()> {
        let bundle = FileSystemBundler::bundle::<M>(manifest_path).await?;

        tokio::fs::create_dir_all(
            bundle_path
                .as_ref()
                .parent()
                .ok_or_else(|| MrBundleError::ParentlessPath(bundle_path.as_ref().to_path_buf()))?,
        )
        .await
        .map_err(|e| {
            MrBundleError::IoError(
                format!(
                    "Failed to create bundle directory: {}",
                    bundle_path.as_ref().display()
                ),
                e,
            )
        })?;

        let bundle_path = bundle_path.as_ref();
        tokio::fs::write(bundle_path, bundle.pack()?)
            .await
            .map_err(|e| {
                MrBundleError::IoError(
                    format!("Failed to write bundle to path: {}", bundle_path.display()),
                    e,
                )
            })?;

        Ok(())
    }

    /// Load a bundle from the filesystem.
    ///
    /// The bundle is automatically unpacked into a [`Bundle`] object.
    pub async fn load_from<M: Debug + Serialize + DeserializeOwned>(
        bundle_path: impl AsRef<Path>,
    ) -> MrBundleResult<Bundle<M>> {
        let bundle_path = bundle_path.as_ref();
        let bundle_bytes = tokio::fs::read(bundle_path).await.map_err(|e| {
            MrBundleError::IoError(format!("Failed to read bundle file: {:?}", bundle_path), e)
        })?;
        Bundle::unpack(&bundle_bytes[..])
    }

    /// Write the contents of the bundle to the filesystem.
    ///
    /// This will create a directory at `target_dir` and write the manifest and resources to it.
    ///
    /// By default, the function will error if the directory already exists. You can override this
    /// by passing `force` with the value `true`.
    pub async fn expand_to<M: Manifest>(
        bundle: &Bundle<M>,
        target_dir: impl AsRef<Path>,
        force: bool,
    ) -> MrBundleResult<()> {
        FileSystemBundler::expand_named_to(bundle, M::file_name(), target_dir, force).await
    }

    /// Write the contents of the bundle to the filesystem.
    ///
    /// This version of the [expand_to](FileSystemBundler::expand_to) has looser constraints on the
    /// contents of the manifest. As a consequence, the file name for the manifest must be provided.
    pub async fn expand_named_to<M: Debug + Serialize + DeserializeOwned>(
        bundle: &Bundle<M>,
        manifest_file_name: &str,
        target_dir: impl AsRef<Path>,
        force: bool,
    ) -> MrBundleResult<()> {
        let target_dir = target_dir.as_ref();

        // If the directory already exists, and we're not forcing, then we can't continue.
        if !force && target_dir.exists() {
            return Err(MrBundleError::DirectoryExists(target_dir.to_owned()));
        }

        // Create the directory to work into.
        tokio::fs::create_dir_all(&target_dir).await.map_err(|e| {
            MrBundleError::IoError(
                format!("Failed to create target directory: {:?}", target_dir),
                e,
            )
        })?;

        // Write the manifest to the target directory.
        let yaml_str = serde_yaml::to_string(bundle.manifest())?;
        let manifest_path = target_dir.join(manifest_file_name);
        tokio::fs::write(&manifest_path, yaml_str.as_bytes())
            .await
            .map_err(|e| {
                MrBundleError::IoError(
                    format!("Failed to write manifest to path: {:?}", manifest_path),
                    e,
                )
            })?;

        // Write the resources to the target directory.
        for (resource_id, resource) in bundle.get_all_resources() {
            let path = target_dir.join(resource_id);
            let path_clone = path.clone();
            let parent = path_clone
                .parent()
                .ok_or_else(|| MrBundleError::ParentlessPath(path.clone()))?;
            tokio::fs::create_dir_all(&parent).await.map_err(|e| {
                MrBundleError::IoError(
                    format!("Failed to create resource directory: {:?}", parent),
                    e,
                )
            })?;
            tokio::fs::write(&path, resource).await.map_err(|e| {
                MrBundleError::IoError(format!("Failed to write resource to path: {:?}", path), e)
            })?;
        }

        Ok(())
    }
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
        assert_eq!(
            "hello.txt",
            resource_id_for_path("../../dir/hello.txt").unwrap()
        );

        // Relative file path should become the file name
        assert_eq!("hello.txt", resource_id_for_path("./hello.txt").unwrap());
    }
}
