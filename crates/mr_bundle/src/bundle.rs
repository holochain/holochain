use crate::manifest::ResourceIdentifier;
use crate::{
    error::{BundleError, MrBundleResult},
    manifest::Manifest,
};
use resource::ResourceBytes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Read;

pub mod resource;

/// A map from resource identifiers to their value as byte arrays.
pub type ResourceMap = HashMap<ResourceIdentifier, ResourceBytes>;

/// A [Manifest], bundled with the Resources that it describes.
///
/// This is meant to be serialized for standalone distribution, and deserialized
/// by the receiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle<M>
where
    M: Serialize + DeserializeOwned,
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
    M: Serialize + DeserializeOwned,
{
    /// Accessor for the Manifest
    pub fn manifest(&self) -> &M {
        &self.manifest
    }

    /// Accessor for the map of resources included in this bundle
    pub fn get_all_resources(&self) -> &ResourceMap {
        &self.resources
    }

    /// Retrieve the bytes for a single resource.
    pub async fn get_resource(
        &self,
        resource_identifier: &ResourceIdentifier,
    ) -> Option<&ResourceBytes> {
        self.resources.get(resource_identifier)
    }

    /// Pack this bundle into a byte array.
    ///
    /// Uses [`pack`](crate::pack) to produce the byte array.
    pub fn pack(&self) -> MrBundleResult<bytes::Bytes> {
        crate::pack(self)
    }

    /// Unpack bytes produced by [`pack`](Bundle::pack) into a new [Bundle].
    ///
    /// Uses [`unpack`](crate::unpack) to produce the new Bundle.
    pub fn unpack(source: impl Read) -> MrBundleResult<Self> {
        crate::unpack(source)
    }
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
            .difference(&resources.keys().cloned().collect())
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

    /// Return a new Bundle with an updated manifest, subject to the same
    /// validation constraints as creating a new Bundle from scratch.
    pub fn update_manifest(self, manifest: M) -> MrBundleResult<Self> {
        Self::from_parts(manifest, self.resources)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MrBundleError;
    use bytes::Buf;

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestManifest(Vec<ResourceIdentifier>);

    impl Manifest for TestManifest {
        fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String> {
            self.resource_ids()
                .iter()
                .map(|r| (r.clone(), r.clone()))
                .collect()
        }

        fn resource_ids(&self) -> Vec<ResourceIdentifier> {
            self.0.clone()
        }

        #[cfg(feature = "fs")]
        #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
        fn file_name() -> &'static str {
            unimplemented!()
        }

        #[cfg(feature = "fs")]
        #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
        fn bundle_extension() -> &'static str {
            unimplemented!()
        }
    }

    #[test]
    fn bundle_validation() {
        let manifest = TestManifest(vec!["1.thing".into(), "2.thing".into()]);

        Bundle::new(
            manifest.clone(),
            vec![
                ("1.thing".into(), vec![1].into()),
                ("2.thing".into(), vec![2].into()),
            ],
        )
        .unwrap();

        let err =
            Bundle::new(manifest.clone(), vec![("1.thing".into(), vec![1].into())]).unwrap_err();
        assert!(
            matches!(err, MrBundleError::BundleError(BundleError::MissingResources(ref resources)) if resources.contains(&"2.thing".into())),
            "Got other error: {err:?}"
        );

        let err = Bundle::new(
            manifest,
            vec![
                ("1.thing".into(), vec![1].into()),
                ("2.thing".into(), vec![2].into()),
                ("3.thing".into(), vec![3].into()),
            ],
        )
        .unwrap_err();
        assert!(
            matches!(
                err,
                MrBundleError::BundleError(BundleError::UnusedResources(ref resources)) if resources.contains(&"3.thing".into())
            ),
            "Got other error: {err:?}"
        );
    }

    #[test]
    fn round_trip_pack_unpack() {
        let manifest = TestManifest(vec!["1.thing".into(), "2.thing".into()]);

        let bundle = Bundle::new(
            manifest.clone(),
            vec![
                ("1.thing".into(), vec![1].into()),
                ("2.thing".into(), vec![2].into()),
            ],
        )
        .unwrap();

        let packed = bundle.pack().unwrap();
        let unpacked = Bundle::unpack(packed.reader()).unwrap();

        assert_eq!(bundle, unpacked);
    }
}
