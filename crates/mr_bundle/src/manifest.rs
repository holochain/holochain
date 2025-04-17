use std::collections::HashMap;

/// The identifier for a resource in the manifest.
pub type ResourceIdentifier = String;

/// A Manifest describes the resources in a [`Bundle`](crate::Bundle) and how
/// to pack and unpack them.
///
/// Regardless of the format of your Manifest, it must contain a set of Locations
/// describing where to find resources, and this trait must implement `locations`
/// properly to match the data contained in the manifest.
///
/// You must also specify a relative path for the Manifest, and the extension
/// for the bundle file, if you are using the "fs" feature.
pub trait Manifest:
    Clone + Sized + PartialEq + Eq + serde::Serialize + serde::de::DeserializeOwned
{
    /// Ask the manifest to produce resources ids and a locator for the resources.
    ///
    /// This operation is required to be idempotent if it is called multiple times. The first
    /// call is expected to mutate the manifest so that its resources refer to ids instead of the
    /// original resource locators. If called again, it can't return useful locators but the ids
    /// must be the same as the first call.
    fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String>;

    /// The list of Locations referenced in the manifest data. This must be
    /// correctly implemented to enable resource resolution.
    fn resource_ids(&self) -> Vec<ResourceIdentifier>;

    /// The file name of the manifest, to be used when unpacking a bundle and as a default when
    /// packaging a from the file system.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    fn file_name() -> &'static str;

    /// When a bundle is created from the filesystem, the bundle file gets this extension.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    fn bundle_extension() -> &'static str;
}
