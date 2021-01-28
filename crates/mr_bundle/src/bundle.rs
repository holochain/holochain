use serde::{Deserialize, Serialize};

use crate::{
    error::{BundleError, BundleResult, MrBundleError, MrBundleResult},
    location::Location,
    manifest::Manifest,
    resource::Resource,
};
use std::{
    collections::{HashMap, HashSet},
    convert::TryFrom,
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
#[derive(Debug, PartialEq, Eq)]
pub struct Bundle<M, R>
where
    M: Manifest,
    R: Resource,
{
    manifest: M,
    resources: HashMap<Location, R>,
}

#[derive(Serialize, Deserialize)]
struct BundleSerialized {
    #[serde(with = "serde_bytes")]
    manifest: Vec<u8>,
    #[serde(with = "serde_bytes")]
    resources: Vec<u8>,
}

impl<M, R> TryFrom<&Bundle<M, R>> for BundleSerialized
where
    M: Manifest,
    R: Resource,
{
    type Error = MrBundleError;
    fn try_from(bundle: &Bundle<M, R>) -> MrBundleResult<BundleSerialized> {
        Ok(Self {
            manifest: crate::encode(&bundle.manifest)?,
            resources: crate::encode(&bundle.resources)?,
        })
    }
}

impl<M, R> TryFrom<&BundleSerialized> for Bundle<M, R>
where
    M: Manifest,
    R: Resource,
{
    type Error = MrBundleError;
    fn try_from(bundle: &BundleSerialized) -> MrBundleResult<Bundle<M, R>> {
        Ok(Self {
            manifest: crate::decode(&bundle.manifest)?,
            resources: crate::decode(&bundle.resources)?,
        })
    }
}

impl<M, R> Bundle<M, R>
where
    M: Manifest,
    R: Resource,
{
    /// Creates a bundle containing a manifest and a collection of resources to
    /// be bundled together with the manifest.
    ///
    /// The paths paired with each resource must correspond to the set of
    /// `Location::Bundle`s specified in the `Manifest::location()`, or else
    /// this is not a valid bundle.
    pub fn new(manifest: M, resources: Vec<(PathBuf, R)>) -> BundleResult<Self> {
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
                return Err(BundleError::BundledPathNotInManifest(resource_path.clone()));
            }
        }

        let resources = resources
            .into_iter()
            .map(|(path, res)| (Location::Bundled(path), res))
            .collect();

        Ok(Self {
            manifest,
            resources,
        })
    }

    pub fn from_file_content(content: &[u8]) -> MrBundleResult<Self> {
        let data: BundleSerialized = crate::decode(content)?;
        Self::try_from(&data)
    }

    pub fn to_file_content(&self) -> MrBundleResult<Vec<u8>> {
        let data = BundleSerialized::try_from(self)?;
        crate::encode(&data)
    }

    pub fn read_from_file(path: &Path) -> MrBundleResult<Self> {
        Ok(Self::from_file_content(&std::fs::read(path)?)?)
    }

    pub fn write_to_file(&self, path: &Path) -> MrBundleResult<()> {
        Ok(std::fs::write(path, self.to_file_content()?)?)
    }

    pub async fn resolve(&self, location: &Location) -> MrBundleResult<R> {
        Ok(match location {
            Location::Bundled(path) => self
                .resources
                .get(location)
                .cloned()
                .ok_or_else(|| BundleError::BundledResourceMissing(path.clone()))?,
            Location::Path(path) => crate::decode(&crate::location::resolve_local(path).await?)?,
            Location::Url(url) => crate::decode(&crate::location::resolve_remote(url).await?)?,
        })
    }

    /// Return the full set of resources specified by this bundle's manifest.
    /// Bundled resources can be returned directly, while all others will be
    /// fetched from the filesystem or the internet.
    pub async fn resolve_all(&self) -> MrBundleResult<HashMap<Location, R>> {
        let resources: HashMap<Location, R> = futures::future::join_all(
            self.manifest
                .locations()
                .into_iter()
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
    pub fn bundled_resources(&self) -> &HashMap<Location, R> {
        &self.resources
    }

    /// An arbitrary and opaque encoding of the bundle data into a byte array
    // NB: Ideally, Bundle could just implement serde Serialize/Deserialize,
    // but the generic types cause problems
    pub fn encode(&self) -> MrBundleResult<Vec<u8>> {
        crate::encode(&(
            crate::encode(&self.manifest)?,
            crate::encode(&self.resources)?,
        ))
    }

    /// Decode bytes produced by `to_bytes`
    pub fn decode(bytes: &[u8]) -> MrBundleResult<Self> {
        let (m, r): (Vec<u8>, Vec<u8>) = crate::decode(bytes)?;
        Ok(Self {
            manifest: crate::decode(&m)?,
            resources: crate::decode(&r)?,
        })
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
    }

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct Thing(u32);

    #[tokio::test]
    async fn bundle_validation() {
        let manifest = TestManifest(vec![
            Location::Bundled("1.thing".into()),
            Location::Bundled("2.thing".into()),
        ]);
        assert!(Bundle::new(manifest.clone(), vec![("1.thing".into(), Thing(1))]).is_ok());

        assert_eq!(
            Bundle::new(manifest, vec![("3.thing".into(), Thing(3))]),
            Err(BundleError::BundledPathNotInManifest("3.thing".into()))
        );
    }
}
