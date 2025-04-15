use std::path::PathBuf;

use crate::location::Location;

pub type ResourceIdentifier = String;

/// A Manifest describes the resources in a [`Bundle`](crate::Bundle) and how
/// to pack and unpack them.
///
/// Regardless of the format of your Manifest, it must contain a set of Locations
/// describing where to find resources, and this trait must implement `locations`
/// properly to match the data contained in the manifest.
///
/// You must also specify a relative path for the Manifest, and the extension
/// for the bundle file, if you are using the "packing" feature.
pub trait Manifest:
    Clone + Sized + PartialEq + Eq + serde::Serialize + serde::de::DeserializeOwned
{
    /// The list of Locations referenced in the manifest data. This must be
    /// correctly implemented to enable resource resolution.
    fn resource_ids(&self) -> Vec<ResourceIdentifier>;

    /// The file name of the manifest, to be used when unpacking a bundle and as a default when
    /// packing a bundle.
    #[cfg(feature = "packing")]
    fn file_name() -> String;

    /// When packing a bundle from a directory structure, the bundle file gets
    /// this extension.
    #[cfg(feature = "packing")]
    fn bundle_extension() -> &'static str;
}
