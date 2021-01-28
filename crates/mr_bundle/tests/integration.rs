use std::{collections::HashSet, path::PathBuf};

use mr_bundle::{bundle::Bundle, location::Location, manifest::Manifest};

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
async fn resource_resolution() -> anyhow::Result<()> {
    let dir = tempdir::TempDir::new("mr_bundle")?;

    // Write a Resource to disk
    let local_thing = Thing("local".into());
    let local_path = dir.path().join("local.thing");
    std::fs::write(&local_path, mr_bundle::encode(&local_thing)?)?;

    let bundled_path = PathBuf::from("bundled.thing");
    let bundled_thing = Thing("bundled".into());

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
    let bundle = Bundle::new(manifest, vec![(bundled_path, bundled_thing.clone())]).unwrap();
    assert_eq!(
        bundle
            .bundled_resources()
            .iter()
            .collect::<HashSet<(&Location, &Thing)>>(),
        maplit::hashset![(&bundled_location, &bundled_thing)]
    );

    assert_eq!(
        bundle
            .resolve_all()
            .await
            .unwrap()
            .iter()
            .collect::<HashSet<(&Location, &Thing)>>(),
        maplit::hashset![
            (&bundled_location, &bundled_thing),
            (&local_location, &local_thing)
        ]
    );

    // Ensure that bundle writing and reading are inverses
    bundle
        .write_to_file(&dir.path().join("bundle.bundle"))
        .unwrap();
    let bundle_file = Bundle::read_from_file(&dir.path().join("bundle.bundle")).unwrap();
    assert_eq!(bundle, bundle_file);

    // Ensure that the bundle is serializable and writable
    let bundled_path = dir.path().join("test.bundle");
    let bundle_bytes = bundle.encode().unwrap();
    std::fs::write(&bundled_path, bundle_bytes)?;

    // Ensure that it is also readable and deserializable
    let decoded_bundle: Bundle<_, _> = Bundle::decode(&std::fs::read(bundled_path)?)?;
    assert_eq!(bundle, decoded_bundle);

    Ok(())
}
