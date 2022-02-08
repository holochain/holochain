use mr_bundle::{Bundle, Location, Manifest};
use std::{collections::HashSet, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
enum TestManifest {
    #[serde(rename = "1")]
    #[serde(alias = "\"1\"")]
    V1(ManifestV1),
}

impl Manifest for TestManifest {
    fn locations(&self) -> Vec<Location> {
        match self {
            Self::V1(mani) => mani.things.iter().map(|b| b.location.clone()).collect(),
        }
    }

    #[cfg(feature = "packing")]
    fn path() -> PathBuf {
        "test-manifest.yaml".into()
    }

    #[cfg(feature = "packing")]
    fn bundle_extension() -> &'static str {
        unimplemented!()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ManifestV1 {
    name: String,
    things: Vec<ThingManifest>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ThingManifest {
    #[serde(flatten)]
    location: Location,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
struct Thing(String);

#[tokio::test]
async fn resource_resolution() {
    let dir = tempfile::tempdir().unwrap();

    // Write a ResourceBytes to disk
    let local_thing = Thing("local".into());
    let local_thing_encoded = mr_bundle::encode(&local_thing).unwrap();
    let local_path = dir.path().join("deeply/nested/local.thing");
    std::fs::create_dir_all(local_path.parent().unwrap()).unwrap();
    std::fs::write(&local_path, mr_bundle::encode(&local_thing).unwrap()).unwrap();

    let bundled_thing = Thing("bundled".into());
    let bundled_thing_encoded = mr_bundle::encode(&bundled_thing).unwrap();
    let bundled_path = PathBuf::from("another/nested/bundled.thing");

    // Create a Manifest that references these resources
    let bundled_location = Location::Bundled(bundled_path.clone());
    let local_location = Location::Path(local_path.clone());
    let manifest = TestManifest::V1(ManifestV1 {
        name: "name".to_string(),
        things: vec![
            ThingManifest {
                location: bundled_location.clone(),
            },
            ThingManifest {
                location: local_location.clone(),
            },
        ],
    });

    // Put the bundled resource into a Bundle (excluding the local resource)
    let bundle = Bundle::new_unchecked(
        manifest,
        vec![(bundled_path.clone(), bundled_thing_encoded.clone())],
    )
    .unwrap();
    assert_eq!(
        bundle
            .bundled_resources()
            .iter()
            .collect::<HashSet<(&PathBuf, &Vec<u8>)>>(),
        maplit::hashset![(&bundled_path, &bundled_thing_encoded)]
    );

    assert_eq!(
        bundle
            .resolve_all_cloned()
            .await
            .unwrap()
            .into_iter()
            .collect::<HashSet<(Location, Vec<u8>)>>(),
        maplit::hashset![
            (bundled_location, bundled_thing_encoded),
            (local_location, local_thing_encoded)
        ]
    );

    // Ensure that the bundle is serializable and writable
    let bundled_path = dir.path().join("test.bundle");
    let bundle_bytes = bundle.encode().unwrap();
    std::fs::write(&bundled_path, bundle_bytes).unwrap();

    // Ensure that it is also readable and deserializable
    let decoded_bundle: Bundle<_> = Bundle::decode(&std::fs::read(bundled_path).unwrap()).unwrap();
    assert_eq!(bundle, decoded_bundle);

    // Ensure that bundle writing and reading are inverses
    bundle
        .write_to_file(&dir.path().join("bundle.bundle"))
        .await
        .unwrap();
    let bundle_file = Bundle::read_from_file(&dir.path().join("bundle.bundle"))
        .await
        .unwrap();
    assert_eq!(bundle, bundle_file);
}

#[cfg(feature = "packing")]
#[tokio::test]
async fn unpack_roundtrip() {
    let dir = tempfile::tempdir().unwrap();

    // Write a ResourceBytes to disk
    let local_thing = Thing("local".into());
    let local_path = dir.path().join("deeply/nested/local.thing");
    std::fs::create_dir_all(local_path.parent().unwrap()).unwrap();
    std::fs::write(&local_path, mr_bundle::encode(&local_thing).unwrap()).unwrap();

    let bundled_thing = Thing("bundled".into());
    let bundled_thing_encoded = mr_bundle::encode(&bundled_thing).unwrap();
    let bundled_path = PathBuf::from("another/nested/bundled.thing");

    // Create a Manifest that references these resources
    let bundled_location = Location::Bundled(bundled_path.clone());
    let local_location = Location::Path(local_path.clone());
    let manifest = TestManifest::V1(ManifestV1 {
        name: "name".to_string(),
        things: vec![
            ThingManifest {
                location: bundled_location.clone(),
            },
            ThingManifest {
                location: local_location.clone(),
            },
        ],
    });

    let unpacked_dir = dir.path().join("unpacked");

    // Put the bundled resource into a Bundle (excluding the local resource)
    let bundle = Bundle::new(
        manifest,
        vec![(bundled_path.clone(), bundled_thing_encoded.clone())],
        unpacked_dir.clone(),
    )
    .unwrap();
    assert_eq!(
        bundle
            .bundled_resources()
            .iter()
            .collect::<HashSet<(&PathBuf, &Vec<u8>)>>(),
        maplit::hashset![(&bundled_path, &bundled_thing_encoded)]
    );

    // Unpack the bundle to a directory on the filesystem
    bundle.unpack_yaml(&unpacked_dir, false).await.unwrap();

    assert!(unpacked_dir.join("test-manifest.yaml").is_file());
    assert!(unpacked_dir.join("another/nested/bundled.thing").is_file());
    assert!(!unpacked_dir.join("deeply/nested/local.thing").exists());

    let reconstructed = Bundle::<TestManifest>::pack_yaml(&unpacked_dir.join("test-manifest.yaml"))
        .await
        .unwrap();

    assert_eq!(bundle, reconstructed);
}
