// use crate::{
//     error::{BundleError, MrBundleResult},
//     ResourceBytes,
// };
// use holochain_util::ffs;
// use std::path::{Path, PathBuf};
//
// /// Where to find a Resource.
// ///
// /// This representation, with named fields, is chosen so that in the yaml config
// /// either "path", "url", or "bundled" can be specified due to this field
// /// being flattened.
// #[derive(Clone, Debug, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// #[serde(rename_all = "snake_case")]
// pub enum Location {
//     /// Expect file to be part of this bundle
//     Bundled(PathBuf),
//
//     /// Get file from local filesystem (not bundled)
//     Path(PathBuf),
// }
//
// impl Location {
//     /// Make a relative Path absolute if possible, given the `root_dir`
//     pub fn normalize(&self, root_dir: Option<&PathBuf>) -> MrBundleResult<Location> {
//         if let Location::Path(path) = self {
//             if path.is_relative() {
//                 if let Some(dir) = root_dir {
//                     Ok(Location::Path(ffs::sync::canonicalize(dir.join(path))?))
//                 } else {
//                     Err(BundleError::RelativeLocalPath(path.to_owned()).into())
//                 }
//             } else {
//                 Ok(self.clone())
//             }
//         } else {
//             Ok(self.clone())
//         }
//     }
// }
//
// pub(crate) async fn resolve_local(path: &Path) -> MrBundleResult<ResourceBytes> {
//     Ok(ffs::read(path).await?.into())
// }
//
// #[cfg(test)]
// mod tests {
//     use super::*;
//     use serde::{Deserialize, Serialize};
//     use serde_yaml::value::{Tag, TaggedValue};
//
//     #[derive(Serialize, Deserialize)]
//     struct TunaSalad {
//         celery: Vec<Location>,
//
//         #[serde(flatten)]
//         mayo: Location,
//     }
//
//     /// Test that Location serializes in a convenient way suitable for
//     /// human-readable manifests, e.g. YAML
//     ///
//     /// The YAML produced by this test looks like:
//     /// ```yaml
//     /// ---
//     /// celery:
//     ///   - !bundled: b
//     ///   - !path: p
//     /// path: "./my-path"
//     /// ```
//     #[test]
//     fn location_flattening() {
//         use serde_yaml::Value;
//
//         let tuna = TunaSalad {
//             celery: vec![Location::Bundled("b".into()), Location::Path("p".into())],
//             mayo: Location::Path("./my-path".into()),
//         };
//         let val = serde_yaml::to_value(&tuna).unwrap();
//         println!("yaml produced:\n{}", serde_yaml::to_string(&tuna).unwrap());
//
//         assert_eq!(
//             val["celery"][0],
//             Value::Tagged(Box::new(TaggedValue {
//                 tag: Tag::new("!bundled"),
//                 value: Value::from("b")
//             }))
//         );
//         assert_eq!(
//             val["celery"][1],
//             Value::Tagged(Box::new(TaggedValue {
//                 tag: Tag::new("!path"),
//                 value: Value::from("p")
//             }))
//         );
//         assert_eq!(val["path"], Value::from("./my-path"));
//     }
// }
