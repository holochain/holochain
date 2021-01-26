use bytes::Bytes;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{error::MrBundleResult, location::Location, manifest::BundleManifest};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, derive_more::From, derive_more::AsRef)]
pub struct Blob(#[serde(with = "serde_bytes")] Vec<u8>);

#[derive(Serialize, Deserialize)]
pub struct Bundle<M, R>
where
    M: BundleManifest,
    R: Serialize + DeserializeOwned,
{
    manifest: M,
    resources: HashMap<Location, R>,
}

impl<M, R> Bundle<M, R>
where
    M: BundleManifest,
    R: Serialize + DeserializeOwned,
{
    pub async fn from_manifest(manifest: M) -> MrBundleResult<Self> {
        let resources: HashMap<Location, R> =
            futures::future::join_all(manifest.locations().into_iter().map(|loc| async move {
                Ok((
                    loc.clone(),
                    crate::decode(&loc.resolve().await?.into_iter().collect::<Vec<u8>>())?,
                ))
            }))
            .await
            .into_iter()
            .collect::<MrBundleResult<HashMap<_, _>>>()?;

        Ok(Self {
            manifest,
            resources,
        })
    }

    pub fn resource(&self, location: &Location) -> Option<&R> {
        self.resources.get(location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_test() {
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "manifest_version")]
        #[allow(missing_docs)]
        enum Manifest {
            #[serde(rename = "1")]
            #[serde(alias = "\"1\"")]
            V1(ManifestV1),
        }

        impl BundleManifest for Manifest {
            fn locations(&self) -> Vec<Location> {
                match self {
                    Self::V1(mani) => mani.things.iter().map(|b| b.location.clone()).collect(),
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct ManifestV1 {
            name: String,
            things: Vec<ThingManifest>,
        }

        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct ThingManifest {
            #[serde(flatten)]
            location: Location,
        }

        #[derive(Serialize, Deserialize)]
        struct Thing(u32);

        let location1 = Location::Bundled("./1.thing".into());
        let location2 = Location::Bundled("./2.thing".into());

        let manifest = Manifest::V1(ManifestV1 {
            name: "name".to_string(),
            things: vec![
                ThingManifest {
                    location: location1.clone(),
                },
                ThingManifest {
                    location: location2.clone(),
                },
            ],
        });

        let bundle = Bundle {
            manifest,
            resources: maplit::hashmap! {
                location1 => Thing(1),
                location2 => Thing(2),
            },
        };

        println!("{}", serde_yaml::to_string(&bundle).unwrap());
    }
}
