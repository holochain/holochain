use crate::manifest::ResourceIdentifier;
use crate::{
    error::{BundleError, MrBundleResult},
    location::Location,
    manifest::Manifest,
    resource::ResourceBytes,
};
use holochain_util::ffs;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

pub type ResourceMap = HashMap<ResourceIdentifier, ResourceBytes>;

/// A Manifest bundled with the Resources that it describes.
///
/// This is meant to be serialized for standalone distribution, and deserialized
/// by the receiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle<M>
where
    M: Manifest,
{
    /// The manifest describing the resources that compose this bundle.
    #[serde(bound(deserialize = "M: DeserializeOwned"))]
    manifest: M,

    /// The full or partial resource data. Each entry must correspond to one
    /// of the Bundled Locations specified by the Manifest. Bundled Locations
    /// are always relative paths (relative to the root_dir).
    resources: ResourceMap,
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
    /// resolved into absolute ones.
    pub fn new(
        manifest: M,
        resources: impl IntoIterator<Item = (ResourceIdentifier, ResourceBytes)>,
    ) -> MrBundleResult<Self> {
        Self::from_parts(manifest, resources)
    }

    fn from_parts(
        manifest: M,
        resources: impl IntoIterator<Item = (ResourceIdentifier, ResourceBytes)>,
    ) -> MrBundleResult<Self> {
        let resources = resources.into_iter().collect::<ResourceMap>();
        let manifest_resource_ids: HashSet<_> = manifest.resource_ids().into_iter().collect();

        let missing_resources = manifest_resource_ids
            .difference(resources.keys().cloned().collect())
            .cloned()
            .collect::<Vec<_>>();
        if !missing_resources.is_empty() {
            return Err(BundleError::MissingResources(missing_resources).into());
        }

        let extra_resources = resources
            .keys()
            .cloned()
            .collect::<HashSet<_>>()
            .difference(&manifest_resource_ids)
            .cloned()
            .collect::<Vec<_>>();

        if !extra_resources.is_empty() {
            return Err(BundleError::UnusedResources(extra_resources).into());
        }

        Ok(Self {
            manifest,
            resources,
        })
    }

    /// Accessor for the Manifest
    pub fn manifest(&self) -> &M {
        &self.manifest
    }

    /// Accessor for the map of resources included in this bundle
    pub async fn get_all_resources(&self) -> &ResourceMap {
        &self.resources
    }

    /// Retrieve the bytes for a single resource.
    pub async fn get_resource(
        &self,
        resource_identifier: &ResourceIdentifier,
    ) -> Option<&ResourceBytes> {
        self.resources.get(resource_identifier)
    }

    /// Return a new Bundle with an updated manifest, subject to the same
    /// validation constraints as creating a new Bundle from scratch.
    pub fn update_manifest(self, manifest: M) -> MrBundleResult<Self> {
        Self::from_parts(manifest, self.resources)
    }

    /// Load a Bundle into memory from a file
    pub async fn read_from_file(path: &Path) -> MrBundleResult<Self> {
        Self::decode(ffs::read(path).await?.into())
    }

    /// Write a Bundle to a file
    pub async fn write_to_file(&self, path: &Path) -> MrBundleResult<()> {
        Ok(ffs::write(path, &self.encode()?).await?)
    }

    /// Access the map of resources included in this bundle
    /// Bundled resources are also accessible via `resolve` or `resolve_all`,
    /// but using this method prevents a Clone
    pub fn bundled_resources(&self) -> &ResourceMap {
        &self.resources
    }

    /// An arbitrary and opaque encoding of the bundle data into a byte array
    pub fn encode(&self) -> MrBundleResult<bytes::Bytes> {
        crate::encode(self)
    }

    /// Decode bytes produced by [`encode`](Bundle::encode)
    pub fn decode(bytes: bytes::Bytes) -> MrBundleResult<Self> {
        crate::decode(&bytes)
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
        crate::util::prune_path(path.into(), M::file_name()).map_err(Into::into)
    }
}

/// A manifest bundled together, optionally, with the Resources that it describes.
/// The manifest may be of any format. This is useful for deserializing a bundle of
/// an outdated format, so that it may be modified to fit the supported format.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct RawBundle<M> {
    /// The manifest describing the resources that compose this bundle.
    #[serde(bound(deserialize = "M: DeserializeOwned"))]
    pub manifest: M,

    /// The full or partial resource data. Each entry must correspond to one
    /// of the Bundled Locations specified by the Manifest. Bundled Locations
    /// are always relative paths (relative to the root_dir).
    pub resources: ResourceMap,
}

impl<M: serde::de::DeserializeOwned> RawBundle<M> {
    /// Load a Bundle into memory from a file
    pub async fn read_from_file(path: &Path) -> MrBundleResult<Self> {
        crate::decode(&ffs::read(path).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MrBundleError;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestManifest(Vec<Location>);

    impl Manifest for TestManifest {
        fn resource_ids(&self) -> Vec<Location> {
            self.0.clone()
        }

        #[cfg(feature = "packing")]
        fn file_name() -> String {
            unimplemented!()
        }

        #[cfg(feature = "packing")]
        fn bundle_extension() -> &'static str {
            unimplemented!()
        }
    }

    #[test]
    fn bundle_validation() {
        let manifest = TestManifest(vec![
            Location::Bundled("1.thing".into()),
            Location::Bundled("2.thing".into()),
        ]);
        assert!(
            Bundle::new_unchecked(manifest.clone(), vec![("1.thing".into(), vec![1].into())])
                .is_ok()
        );

        matches::assert_matches!(
            Bundle::new_unchecked(manifest, vec![("3.thing".into(), vec![3].into())]),
            Err(MrBundleError::BundleError(BundleError::BundledPathNotInManifest(path))) if path == PathBuf::from("3.thing")
        );
    }
}
