use crate::{
    error::{BundleError, MrBundleResult},
    location::Location,
    manifest::Manifest,
    resource::ResourceBytes,
};
use holochain_util::ffs;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};

pub type ResourceMap = BTreeMap<PathBuf, ResourceBytes>;

/// A Manifest bundled together, optionally, with the Resources that it describes.
/// This is meant to be serialized for standalone distribution, and deserialized
/// by the receiver.
///
/// The manifest may describe locations of resources not included in the Bundle.
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

    /// Since the Manifest may contain local paths referencing unbundled files,
    /// on the local filesystem, we must have an absolute path at runtime for
    /// normalizing those locations.
    ///
    /// Passing None is a runtime assertion that the manifest contains only
    /// absolute local paths. If this assertion fails,
    /// **resource resolution will panic!**
    //
    // MAYBE: Represent this with types more solidly, perhaps breaking this
    //        struct into two versions for each case.
    #[serde(skip)]
    root_dir: Option<PathBuf>,
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
    pub fn new<R: IntoIterator<Item = (PathBuf, ResourceBytes)>>(
        manifest: M,
        resources: R,
        root_dir: PathBuf,
    ) -> MrBundleResult<Self> {
        Self::from_parts(manifest, resources, Some(root_dir))
    }

    /// Create a bundle, asserting that all paths in the Manifest are absolute.
    pub fn new_unchecked<R: IntoIterator<Item = (PathBuf, ResourceBytes)>>(
        manifest: M,
        resources: R,
    ) -> MrBundleResult<Self> {
        Self::from_parts(manifest, resources, None)
    }

    fn from_parts<R: IntoIterator<Item = (PathBuf, ResourceBytes)>>(
        manifest: M,
        resources: R,
        root_dir: Option<PathBuf>,
    ) -> MrBundleResult<Self> {
        let resources: ResourceMap = resources.into_iter().collect();
        let manifest_paths: HashSet<_> = manifest
            .locations()
            .into_iter()
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
            root_dir,
        })
    }

    /// Accessor for the Manifest
    pub fn manifest(&self) -> &M {
        &self.manifest
    }

    /// Return a new Bundle with an updated manifest, subject to the same
    /// validation constraints as creating a new Bundle from scratch.
    pub fn update_manifest(self, manifest: M) -> MrBundleResult<Self> {
        Self::from_parts(manifest, self.resources, self.root_dir)
    }

    /// Load a Bundle into memory from a file
    pub async fn read_from_file(path: &Path) -> MrBundleResult<Self> {
        Self::decode(ffs::read(path).await?.into())
    }

    /// Write a Bundle to a file
    pub async fn write_to_file(&self, path: &Path) -> MrBundleResult<()> {
        Ok(ffs::write(path, &self.encode()?).await?)
    }

    /// Retrieve the bytes for a resource at a Location, downloading it if
    /// necessary
    pub async fn resolve(&self, location: &Location) -> MrBundleResult<ResourceBytes> {
        let bytes = match &location.normalize(self.root_dir.as_ref())? {
            Location::Bundled(path) => self
                .resources
                .get(path)
                .ok_or_else(|| BundleError::BundledResourceMissing(path.clone()))?
                .clone(),
            Location::Path(path) => crate::location::resolve_local(path).await?,
            Location::Url(url) => crate::location::resolve_remote(url).await?,
        };
        Ok(bytes)
    }

    /// Return the full set of resources specified by this bundle's manifest.
    /// References to bundled resources can be returned directly, while all
    /// others will be fetched from the filesystem or the network.
    pub async fn resolve_all(&self) -> MrBundleResult<HashMap<Location, ResourceBytes>> {
        futures::future::join_all(
            self.manifest.locations().into_iter().map(|loc| async move {
                MrBundleResult::Ok((loc.clone(), self.resolve(&loc).await?))
            }),
        )
        .await
        .into_iter()
        .collect::<MrBundleResult<HashMap<Location, ResourceBytes>>>()
    }

    /// Resolve all resources, but with fully owned references
    pub async fn resolve_all_cloned(&self) -> MrBundleResult<HashMap<Location, ResourceBytes>> {
        Ok(self.resolve_all().await?.into_iter().collect())
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
        crate::util::prune_path(path.into(), M::path()).map_err(Into::into)
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
    use crate::error::MrBundleError;

    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestManifest(Vec<Location>);

    impl Manifest for TestManifest {
        fn locations(&self) -> Vec<Location> {
            self.0.clone()
        }

        #[cfg(feature = "packing")]
        fn path() -> PathBuf {
            unimplemented!()
        }

        #[cfg(feature = "packing")]
        fn bundle_extension() -> &'static str {
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
