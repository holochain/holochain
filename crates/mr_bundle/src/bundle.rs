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
    path::PathBuf,
};

#[derive(Serialize, Deserialize, derive_more::From, derive_more::AsRef)]
pub struct Blob(#[serde(with = "serde_bytes")] Vec<u8>);

/// A Manifest bundled together, optionally, with the Resources that it describes.
/// This is meant to be serialized for standalone distribution, and deserialized
/// by the receiver.
///
/// The manifest may describe locations of resources not included in the Bundle.
//
// TODO: make clear the difference between the partial set of resources and
// the full set of resolved resources.
//
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
    manifest: Vec<u8>,
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

    /// Return the full set of resources specified by this bundle's manifest.
    /// Bundled resources can be returned directly, while all others will be
    /// fetched from the filesystem or the internet.
    pub async fn resolve_all(&self) -> HashMap<Location, R> {
        todo!()
    }

    // TODO: break this up into `resolve_all` and `new` which takes a partial set of
    // bundled resources
    pub async fn from_manifest(manifest: M) -> MrBundleResult<Self> {
        let resources: HashMap<Location, R> =
            futures::future::join_all(manifest.locations().into_iter().map(|loc| async move {
                Ok((
                    loc.clone(),
                    crate::decode(&loc.resolve().await?.into_iter().collect::<Vec<u8>>())?,
                ))
            }))
            .await
            .into_iter()
            .collect::<MrBundleResult<HashMap<_, _>>>()?;

        Ok(Self {
            manifest,
            resources,
        })
    }

    pub fn resources(&self) -> &HashMap<Location, R> {
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
