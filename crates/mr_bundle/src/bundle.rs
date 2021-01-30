use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    error::{BundleError, BundleResult, MrBundleError, MrBundleResult},
    io_error::IoError,
    location::Location,
    manifest::Manifest,
    resource::ResourceBytes,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

/// A Manifest bundled together, optionally, with the Resources that it describes.
/// This is meant to be serialized for standalone distribution, and deserialized
/// by the receiver.
///
/// The manifest may describe locations of resources not included in the Bundle.
///
// NB: It would be so nice if this were Deserializable, but there are problems
// with using the derive macro here.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle<M>
where
    M: Manifest,
{
    #[serde(bound(deserialize = "M: DeserializeOwned"))]
    manifest: M,
    resources: HashMap<PathBuf, ResourceBytes>,
    #[serde(skip)]
    normalized_locations: Vec<Location>,
}

impl<M> Bundle<M>
where
    M: Manifest,
{
    /// Creates a bundle containing a manifest and a collection of resources to
    /// be bundled together with the manifest.
    ///
    /// The paths paired with each resource must correspond to the set of
    /// `Location::Bundle`s specified in the `Manifest::location()`, or else
    /// this is not a valid bundle.
    ///
    /// A base directory must also be supplied so that relative paths can be
    /// resolved into absolute ones
    pub fn new(
        manifest: M,
        resources: Vec<(PathBuf, ResourceBytes)>,
        base_dir: &Path,
    ) -> MrBundleResult<Self> {
        Self::from_parts(manifest, resources, Some(base_dir))
    }

    /// Create a bundle, but without
    pub fn new_unchecked(
        manifest: M,
        resources: Vec<(PathBuf, ResourceBytes)>,
    ) -> MrBundleResult<Self> {
        Self::from_parts(manifest, resources, None)
    }

    pub fn from_parts(
        manifest: M,
        resources: Vec<(PathBuf, ResourceBytes)>,
        base_dir: Option<&Path>,
    ) -> MrBundleResult<Self> {
        let normalized_locations = Self::normalize_locations(manifest.locations(), base_dir)?;
        let manifest_paths: HashSet<_> = normalized_locations
            .iter()
            .filter_map(|loc| match loc {
                Location::Bundled(path) => Some(path),
                _ => None,
            })
            .collect();

        // Validate that each resource path is contained in the manifest
        for (resource_path, _) in resources.iter() {
            if !manifest_paths.contains(resource_path) {
                return Err(BundleError::BundledPathNotInManifest(resource_path.clone()).into());
            }
        }

        let resources = resources.into_iter().collect();
        Ok(Self {
            manifest,
            resources,
            normalized_locations,
        })
    }

    pub fn manifest(&self) -> &M {
        &self.manifest
    }

    pub async fn read_from_file(path: &Path) -> MrBundleResult<Self> {
        Ok(Self::decode(&crate::fs(path).read().await?)?)
    }

    pub async fn write_to_file(&self, path: &Path) -> MrBundleResult<()> {
        Ok(crate::fs(path).write(&self.encode()?).await?)
    }

    pub async fn resolve(&self, location: &Location) -> MrBundleResult<ResourceBytes> {
        let bytes = match location {
            Location::Bundled(path) => self
                .resources
                .get(path)
                .cloned()
                .ok_or_else(|| BundleError::BundledResourceMissing(path.clone()))?,
            Location::Path(path) => crate::location::resolve_local(path).await?,
            Location::Url(url) => crate::location::resolve_remote(url).await?,
        };
        Ok(bytes)
    }

    /// Return the full set of resources specified by this bundle's manifest.
    /// Bundled resources can be returned directly, while all others will be
    /// fetched from the filesystem or the internet.
    pub async fn resolve_all(&self) -> MrBundleResult<HashMap<Location, ResourceBytes>> {
        let resources: HashMap<Location, ResourceBytes> = futures::future::join_all(
            self.normalized_locations
                .iter()
                .map(|loc| async move { Ok((loc.clone(), self.resolve(&loc).await?)) }),
        )
        .await
        .into_iter()
        .collect::<MrBundleResult<HashMap<_, _>>>()?;

        Ok(resources)
    }

    /// Access the map of resources included in this bundle
    /// Bundled resources are also accessible via `resolve` or `resolve_all`,
    /// but using this method prevents a Clone
    pub fn bundled_resources(&self) -> &HashMap<PathBuf, ResourceBytes> {
        &self.resources
    }

    /// An arbitrary and opaque encoding of the bundle data into a byte array
    // NB: Ideally, Bundle could just implement serde Serialize/Deserialize,
    // but the generic types cause problems
    pub fn encode(&self) -> MrBundleResult<Vec<u8>> {
        crate::encode(self)
    }

    /// Decode bytes produced by `to_bytes`
    pub fn decode(bytes: &[u8]) -> MrBundleResult<Self> {
        crate::decode(bytes)
    }

    fn normalize_locations(
        locations: Vec<Location>,
        base_dir: Option<&Path>,
    ) -> MrBundleResult<Vec<Location>> {
        locations
            .into_iter()
            .map(|loc| {
                if let Location::Path(path) = &loc {
                    if path.is_relative() {
                        if let Some(base_dir) = base_dir {
                            Ok(Location::Path(
                                base_dir
                                    .join(&path)
                                    .canonicalize()
                                    .map_err(|e| IoError::new(e, Some(path.to_owned())))?,
                            ))
                        } else {
                            Err(BundleError::RelativeLocalPath(path.to_owned()).into())
                        }
                    } else {
                        Ok(loc)
                    }
                } else {
                    Ok(loc)
                }
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// Given that the Manifest is located at the given absolute `path`, find
    /// the absolute root directory for the "unpacked" Bundle directory.
    /// Useful when "imploding" a directory into a bundle to determine the
    /// default location of the generated Bundle file.
    ///
    /// This will only be different than the Manifest path itself if the
    /// Manifest::path impl specifies a nested path.
    ///
    /// Will return None if the `path` does not actually end with the
    /// manifest relative path, meaning that either the manifest file is
    /// misplaced within the unpacked directory, or an incorrect path was
    /// supplied.
    #[cfg(feature = "packing")]
    pub fn find_root_dir(&self, path: &Path) -> MrBundleResult<PathBuf> {
        crate::util::prune_path(path.into(), self.manifest.path()).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestManifest(Vec<Location>);

    impl Manifest for TestManifest {
        fn locations(&self) -> Vec<Location> {
            self.0.clone()
        }

        #[cfg(feature = "packing")]
        fn path(&self) -> PathBuf {
            unimplemented!()
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Thing(u32);

    #[tokio::test]
    async fn bundle_validation() {
        let manifest = TestManifest(vec![
            Location::Bundled("1.thing".into()),
            Location::Bundled("2.thing".into()),
        ]);
        assert!(Bundle::new_unchecked(manifest.clone(), vec![("1.thing".into(), vec![1])]).is_ok());

        matches::assert_matches!(
            Bundle::new_unchecked(manifest, vec![("3.thing".into(), vec![3])]),
            Err(MrBundleError::BundleError(BundleError::BundledPathNotInManifest(path))) if path == PathBuf::from("3.thing")
        );
    }
}
