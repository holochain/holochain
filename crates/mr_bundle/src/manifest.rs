use std::path::PathBuf;

use crate::location::Location;

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
    fn locations(&self) -> Vec<Location>;

    /// When unpacking the bundle into a directory structure, this becomes
    /// the relative path of the manifest file.
    #[cfg(feature = "packing")]
    fn path() -> PathBuf;

    /// When packing a bundle from a directory structure, the bundle file gets
    /// this extension.
    #[cfg(feature = "packing")]
    fn bundle_extension() -> &'static str;

    /// Get only the Bundled locations
    fn bundled_paths(&self) -> Vec<PathBuf> {
        self.locations()
            .into_iter()
            .filter_map(|loc| {
                if let Location::Bundled(path) = loc {
                    Some(path)
                } else {
                    None
                }
            })
            .collect()
    }
}
