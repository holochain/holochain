use std::collections::HashMap;
use std::fmt::Debug;

/// The identifier for a resource in the manifest.
pub type ResourceIdentifier = String;

/// A Manifest describes the resources in a [`Bundle`](crate::Bundle).
///
/// A manifest implementation is expected to describe a set of resources that
/// it intends to be bundled with. The resources are expected to be identifiable
/// by a [`ResourceIdentifier`], which is a string.
///
/// The bundler uses [`generate_resource_ids`](Manifest::generate_resource_ids) to
/// request that the manifest produce a set of resource ids. The manifest must
/// replace its resource locators with the generated ids and return the pairs of
/// ids and resource locations to the bundler.
pub trait Manifest:
    Clone + Sized + PartialEq + Eq + Debug + serde::Serialize + serde::de::DeserializeOwned
{
    /// Ask the manifest to produce resource ids and a locator for the resources.
``
    ///
    /// After the operations complete, the manifest must have replaced its resource
    /// locators with the generated ids. The returned map must contain the pairs of
    /// resource ids and their original locators.
    ///
    /// This operation is required to be idempotent if it is called multiple times. The first
    /// call is expected to mutate the manifest so that its resources refer to ids instead of the
    /// original resource locators. If called again, it can't return useful locators but the ids
    /// must be the same as the first call.
    fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String>;

    /// The list of resources referenced in the manifest data.
    ///
    /// This must return the same value before or after the call to [`generate_resource_ids`](Manifest::generate_resource_ids).
    fn resource_ids(&self) -> Vec<ResourceIdentifier>;

    /// The file name of the manifest file.
    ///
    /// This is recommended to contain a file extension, but it is not required.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    fn file_name() -> &'static str;

    /// The file extension to use when writing the bundle to the filesystem.
    #[cfg(feature = "fs")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
    fn bundle_extension() -> &'static str;
}
