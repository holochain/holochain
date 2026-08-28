use crate::error::MrBundleError;
use crate::manifest::ResourceIdentifier;
use crate::{error::MrBundleResult, manifest::Manifest};
use resource::ResourceBytes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::io::Read;
use std::path::{Component, Path};

pub mod resource;

/// A map from resource identifiers to their value as byte arrays.
pub type ResourceMap = BTreeMap<ResourceIdentifier, ResourceBytes>;

/// A [Manifest], bundled with the Resources that it describes.
///
/// This is meant to be serialized for standalone distribution, and deserialized
/// by the receiver.
///
/// Resource-identifier safety (rejecting absolute paths and parent-directory
/// traversal) is enforced only by [`Bundle::unpack`], not by this type's
/// [`Deserialize`] impl. Callers that need that guarantee on untrusted bytes
/// must go through [`Bundle::unpack`] rather than deserializing a `Bundle`
/// directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle<M>
where
    M: Debug + Serialize + DeserializeOwned,
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
    M: Debug + Serialize + DeserializeOwned,
{
    /// Accessor for the Manifest
    pub fn manifest(&self) -> &M {
        &self.manifest
    }

    /// Accessor for the map of resources included in this bundle
    pub fn get_all_resources(&self) -> HashMap<&ResourceIdentifier, &ResourceBytes> {
        self.resources.iter().collect()
    }

    /// Retrieve the bytes for a single resource.
    pub fn get_resource(&self, resource_identifier: &ResourceIdentifier) -> Option<&ResourceBytes> {
        self.resources.get(resource_identifier)
    }

    /// Pack this bundle into a byte array.
    ///
    /// Uses [`pack`](fn@crate::pack) to produce the byte array.
    pub fn pack(&self) -> MrBundleResult<bytes::Bytes> {
        crate::pack(self)
    }

    /// Unpack bytes produced by [`pack`](Bundle::pack) into a new [Bundle].
    ///
    /// Uses [`unpack`](crate::unpack) to produce the new bundle.
    ///
    /// # Errors
    ///
    /// Returns [`MrBundleError::IoError`] if decompression fails,
    /// [`MrBundleError::MsgpackDecodeError`] if deserialization fails, or
    /// [`MrBundleError::InvalidResourceId`] if a resource identifier could
    /// escape a consumer's bundle root.
    pub fn unpack(source: impl Read) -> MrBundleResult<Self> {
        let bundle: Self = crate::unpack(source)?;

        for resource_id in bundle.resources.keys() {
            validate_resource_id(resource_id)?;
        }

        Ok(bundle)
    }
}

fn validate_resource_id(resource_id: &ResourceIdentifier) -> MrBundleResult<()> {
    let path = Path::new(resource_id);
    let escapes_root = path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });

    if escapes_root {
        return Err(MrBundleError::InvalidResourceId(resource_id.clone()));
    }

    Ok(())
}

// Same failure mode as `HoloHashed<C>` (`holo_hash::hashed`):
// `WithoutGenerics` substitutes `Dummy` for `M`, which doesn't implement
// `Serialize`/`DeserializeOwned`. Hand-written instead, declared as a
// genuine generic type so ordinary fields like `AppBundle(Bundle<AppManifest>)`
// resolve their `ResourceMap` import correctly.
#[cfg(feature = "ts_rs")]
impl<M> ts_rs::TS for Bundle<M>
where
    M: Debug + Serialize + DeserializeOwned + ts_rs::TS,
{
    type WithoutGenerics = crate::ts::BundleWithoutGenerics;
    type OptionInnerType = Self;

    fn name(cfg: &ts_rs::Config) -> String {
        format!("Bundle<{}>", M::name(cfg))
    }

    fn inline(cfg: &ts_rs::Config) -> String {
        format!("{{ manifest: {}, resources: ResourceMap }}", M::name(cfg))
    }

    fn decl(_: &ts_rs::Config) -> String {
        "type Bundle<M> = { manifest: M, resources: ResourceMap };".into()
    }

    fn decl_concrete(cfg: &ts_rs::Config) -> String {
        format!("type Bundle = {};", <Self as ts_rs::TS>::inline(cfg))
    }

    fn visit_dependencies(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        v.visit::<M>();
        M::visit_dependencies(v);
        v.visit::<crate::ts::ResourceMapTs>();
    }

    fn visit_generics(v: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        M::visit_generics(v);
        v.visit::<M>();
    }

    fn output_path() -> Option<std::path::PathBuf> {
        Some("api/admin/types.ts".into())
    }
}

#[cfg(all(test, feature = "ts_rs"))]
mod ts_tests {
    use super::*;
    use ts_rs::TS;

    #[derive(Debug, Serialize, Deserialize, ts_rs::TS)]
    #[ts(export_to = "api/admin/types.ts")]
    struct TestManifestTs {
        value: String,
    }

    #[test]
    fn name_and_decl_are_a_real_generic_declaration() {
        let cfg = ts_rs::Config::default();

        assert_eq!(
            Bundle::<TestManifestTs>::name(&cfg),
            "Bundle<TestManifestTs>"
        );
        assert_eq!(
            Bundle::<TestManifestTs>::decl(&cfg),
            "type Bundle<M> = { manifest: M, resources: ResourceMap };"
        );
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
        mut manifest: M,
        resources: impl IntoIterator<Item = (ResourceIdentifier, ResourceBytes)>,
    ) -> MrBundleResult<Self> {
        let resources = resources.into_iter().collect::<ResourceMap>();
        let manifest_resource_ids: HashSet<_> =
            manifest.generate_resource_ids().keys().cloned().collect();

        let missing_resources = manifest_resource_ids
            .difference(&resources.keys().cloned().collect())
            .cloned()
            .collect::<Vec<_>>();
        if !missing_resources.is_empty() {
            return Err(MrBundleError::MissingResources(missing_resources));
        }

        let extra_resources = resources
            .keys()
            .cloned()
            .collect::<HashSet<_>>()
            .difference(&manifest_resource_ids)
            .cloned()
            .collect::<Vec<_>>();

        if !extra_resources.is_empty() {
            return Err(MrBundleError::UnusedResources(extra_resources));
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
            matches!(err, MrBundleError::MissingResources(ref resources) if resources.contains(&"2.thing".into())),
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
                MrBundleError::UnusedResources(ref resources) if resources.contains(&"3.thing".into())
            ),
            "Got other error: {err:?}"
        );
    }

    #[test]
    fn round_trip_pack_unpack() {
        let manifest = TestManifest(vec!["1.thing".into(), "nested/2.thing".into()]);

        let bundle = Bundle::new(
            manifest.clone(),
            vec![
                ("1.thing".into(), vec![1].into()),
                ("nested/2.thing".into(), vec![2].into()),
            ],
        )
        .unwrap();

        let packed = bundle.pack().unwrap();
        let unpacked = Bundle::unpack(packed.reader()).unwrap();

        assert_eq!(bundle, unpacked);
    }

    #[test]
    fn round_trip_pack_unpack_with_cur_dir_components() {
        let manifest = TestManifest(vec!["./thing".into(), "nested/./thing".into()]);

        let bundle = Bundle::new(
            manifest.clone(),
            vec![
                ("./thing".into(), vec![1].into()),
                ("nested/./thing".into(), vec![2].into()),
            ],
        )
        .unwrap();

        let packed = bundle.pack().unwrap();
        let unpacked = Bundle::unpack(packed.reader()).unwrap();

        assert_eq!(bundle, unpacked);
    }

    #[test]
    fn consistent_id_generation_in_mem() {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct TestManifest(String);

        impl Manifest for TestManifest {
            fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String> {
                let id = self.0.split(".").last().unwrap().to_string();
                let original = self.0.clone();

                self.0 = id.clone();

                HashMap::from([(id, original)])
            }

            fn resource_ids(&self) -> Vec<ResourceIdentifier> {
                vec![self.0.clone()]
            }

            fn file_name() -> &'static str {
                "test.yaml"
            }

            fn bundle_extension() -> &'static str {
                "test"
            }
        }

        let manifest = TestManifest("test.thing".into());

        let bundle = Bundle::new(manifest.clone(), vec![("thing".into(), vec![1].into())]).unwrap();

        assert_eq!(vec!["thing".to_string()], bundle.manifest.resource_ids());
        assert_eq!(
            &ResourceBytes::from(vec![1]),
            bundle.get_resource(&"thing".into()).unwrap()
        );
    }

    fn packed_bundle_with_resource_id(resource_id: &str) -> bytes::Bytes {
        Bundle::new(
            TestManifest(vec![resource_id.to_string()]),
            [(resource_id.to_string(), vec![1].into())],
        )
        .unwrap()
        .pack()
        .unwrap()
    }

    fn assert_unpack_rejects(resource_id: &str) {
        let packed = packed_bundle_with_resource_id(resource_id);
        let error = Bundle::<TestManifest>::unpack(packed.reader())
            .expect_err("unsafe resource identifier unexpectedly unpacked");

        assert!(
            matches!(&error, MrBundleError::InvalidResourceId(id) if id == resource_id),
            "expected InvalidResourceId({resource_id:?}), got {error:?}"
        );
    }

    #[test]
    fn unpack_rejects_unsafe_resource_ids() {
        for resource_id in ["../outside", "nested/../../outside", "/outside"] {
            assert_unpack_rejects(resource_id);
        }
    }

    #[cfg(windows)]
    #[test]
    fn unpack_rejects_windows_prefixed_resource_ids() {
        for resource_id in [r"C:\outside", r"C:outside"] {
            assert_unpack_rejects(resource_id);
        }
    }
}
