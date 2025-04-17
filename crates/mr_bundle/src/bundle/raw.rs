use std::io::Read;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use crate::error::MrBundleResult;
use crate::ResourceMap;

/// A manifest bundled together with the Resources that it describes.
///
/// The manifest may be of any format. This is useful for deserializing a bundle of
/// an outdated format, so that it may be modified to fit the supported format.
#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct RawBundle<M> {
    /// The manifest describing the resources that compose this bundle.
    #[serde(bound(deserialize = "M: DeserializeOwned"))]
    pub manifest: M,

    /// The resources that are bundled together with the manifest.
    pub resources: ResourceMap,
}

impl<M: DeserializeOwned> RawBundle<M> {
    /// Unpack a raw bundle using [unpack](crate::unpack).
    ///
    /// # Example
    ///
    /// Assuming the manifest is unknown for a bundle, you can unpack it as raw YAML.
    ///
    /// ```rust
    /// use bytes::{Buf, Bytes};
    /// use mr_bundle::RawBundle;
    ///
    /// let packed = Bytes::from_static(&[31, 139, 8, 0, 0, 0, 0, 0, 0, 255, 107, 90, 145, 155, 152, 151, 153, 150, 90, 92, 50, 113, 185, 161, 94, 73, 70, 102, 94, 250, 202, 162, 212, 226, 252, 210, 162, 228, 212, 226, 70, 152, 208, 17, 70, 70, 0, 24, 98, 63, 6, 41, 0, 0, 0]);
    ///
    /// let raw = RawBundle::<serde_yaml::Value>::unpack(packed.reader()).unwrap();
    ///
    /// println!("Raw manifest: {:?}", raw.manifest);
    /// ```
    pub fn unpack(source: impl Read) -> MrBundleResult<Self> {
        crate::unpack(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Buf;
    use serde::{Deserialize, Serialize};
    use crate::{Bundle, Manifest, ResourceIdentifier};

    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct TestManifest(Vec<ResourceIdentifier>);

    impl Manifest for TestManifest {
        fn resource_ids(&self) -> Vec<ResourceIdentifier> {
            self.0.clone()
        }

        #[cfg(feature = "fs")]
        #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
        fn file_name() -> String {
            unimplemented!()
        }

        #[cfg(feature = "fs")]
        #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
        fn bundle_extension() -> &'static str {
            unimplemented!()
        }
    }

    #[test]
    fn unpack_raw_manifest() {
        let manifest = TestManifest(vec!["1.thing".into()]);

        let bundle =
            Bundle::new(manifest.clone(), vec![("1.thing".into(), vec![1].into())]).unwrap();

        let packed = bundle.pack().unwrap();

        println!("Packed: {:?}", packed.iter().as_slice());

        // Unpack while treating the manifest as opaque YAML.
        let out = RawBundle::<serde_yaml::Value>::unpack(packed.reader()).unwrap();

        assert_eq!(serde_yaml::to_value(manifest).unwrap(), out.manifest);
        assert_eq!(bundle.resources, out.resources);
    }
}
