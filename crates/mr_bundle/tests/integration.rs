use mr_bundle::{
    resource_id_for_path, Bundle, Manifest, ResourceBytes, ResourceIdentifier,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
enum TestManifest {
    #[serde(rename = "1")]
    #[serde(alias = "\"1\"")]
    V1(ManifestV1),
}

impl Manifest for TestManifest {
    fn generate_resource_ids(&mut self) -> HashMap<ResourceIdentifier, String> {
        let mut out = HashMap::new();

        match self {
            Self::V1(manifest) => {
                for thing in &mut manifest.things {
                    let id = resource_id_for_path(&thing.location).unwrap();
                    out.insert(id.clone(), thing.location.clone());
                    thing.location = id;
                }
            }
        }

        out
    }

    fn resource_ids(&self) -> Vec<ResourceIdentifier> {
        match self {
            Self::V1(manifest) => manifest
                .things
                .iter()
                .map(|b| resource_id_for_path(&b.location).unwrap())
                .collect(),
        }
    }

    #[cfg(feature = "fs")]
    fn file_name() -> &'static str {
        "test-manifest.yaml"
    }

    #[cfg(feature = "fs")]
    fn bundle_extension() -> &'static str {
        "bundle"
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ManifestV1 {
    name: String,
    things: Vec<ThingManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ThingManifest {
    location: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct Thing(String);

impl From<Thing> for ResourceBytes {
    fn from(thing: Thing) -> Self {
        ResourceBytes::from(thing.0.as_bytes().to_vec())
    }
}

#[cfg(feature = "fs")]
#[tokio::test]
async fn bundle_from_manifest() {
    let dir = tempfile::tempdir().unwrap();

    // Write a ResourceBytes to disk
    let resource: ResourceBytes = Thing("some content".into()).into();
    let resource_path = PathBuf::from("another/nested/bundled.thing");
    tokio::fs::create_dir_all(dir.path().join(resource_path.parent().unwrap()))
        .await
        .unwrap();
    tokio::fs::write(dir.path().join(&resource_path), resource).await.unwrap();

    // Create a Manifest that references these resources
    let manifest = TestManifest::V1(ManifestV1 {
        name: "name".to_string(),
        things: vec![
            ThingManifest {
                location: resource_path.to_str().unwrap().to_string(),
            },
        ],
    });

    let manifest_path = dir.path().join(TestManifest::file_name());
    tokio::fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap()).await.unwrap();

    let bundle: Bundle<TestManifest> = Bundle::pack_from_manifest_path(manifest_path).await.unwrap();

    let bundle_path = dir.path().join(format!("test-bundle.{}", TestManifest::bundle_extension()));
    tokio::fs::write(bundle_path, bundle.pack().unwrap()).await.unwrap();

    // let unpacked_dir = dir_path.join("unpacked");
    //
    // // Put the bundled resource into a Bundle (excluding the local resource)
    // let bundle = Bundle::new(
    //     manifest,
    //     vec![(resource_path.clone(), bundled_thing_encoded.clone())],
    // )
    // .unwrap();
    // assert_eq!(
    //     bundle
    //         .bundled_resources()
    //         .iter()
    //         .collect::<HashSet<(&PathBuf, &ResourceBytes)>>(),
    //     maplit::hashset![(&resource_path, &bundled_thing_encoded)]
    // );
    //
    // // Unpack the bundle to a directory on the filesystem
    // bundle.unpack_to_dir(&unpacked_dir, false).await.unwrap();
    //
    // assert!(unpacked_dir.join("test-manifest.yaml").is_file());
    // assert!(unpacked_dir.join("another/nested/bundled.thing").is_file());
    // assert!(!unpacked_dir.join("deeply/nested/local.thing").exists());
    //
    // let reconstructed =
    //     Bundle::<TestManifest>::pack_from_manifest_path(&unpacked_dir.join("test-manifest.yaml"))
    //         .await
    //         .unwrap();
    //
    // assert_eq!(bundle, reconstructed);
}
