use std::collections::HashMap;

use crate::location::Location;

#[derive(serde::Serialize)]
pub struct Bundle<Manifest, File>
where
    Manifest: serde::Serialize + serde::de::DeserializeOwned,
    File: serde::Serialize + serde::de::DeserializeOwned,
{
    manifest: Manifest,
    files: HashMap<Location, File>,
}

#[cfg(test)]
mod tests {
    use crate::location::Location;

    use super::Bundle;

    #[test]
    fn bundle_test() {
        #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[serde(tag = "manifest_version")]
        #[allow(missing_docs)]
        enum Manifest {
            #[serde(rename = "1")]
            #[serde(alias = "\"1\"")]
            V1(ManifestV1),
        }

        #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        struct ManifestV1 {
            name: String,
            blobs: Vec<BlobManifest>,
        }

        #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        struct BlobManifest {
            #[serde(flatten)]
            location: Location,
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        struct Blob(u32);

        let manifest = Manifest::V1(ManifestV1 {
            name: "name".to_string(),
            blobs: vec![BlobManifest {
                location: Location::Path("./hi.blob".into()),
            }],
        });

        let bundle = Bundle {
            manifest,
            files: maplit::hashmap! {
                Location::Bundled("1.blob".into()) => Blob(1),
                Location::Bundled("2.blob".into()) => Blob(2),
            },
        };

        println!("{}", serde_yaml::to_string(&bundle).unwrap());
    }
}
