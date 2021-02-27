use std::path::PathBuf;

use crate::location::Location;

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
