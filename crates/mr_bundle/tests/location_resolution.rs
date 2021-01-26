use mr_bundle::{bundle::Bundle, location::Location, manifest::BundleManifest};
use std::{env::temp_dir, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Thing(u32);

#[tokio::test]
async fn path_roundtrip() -> anyhow::Result<()> {
    let dir = tempdir::TempDir::new("mr_bundle")?;

    // Write some Resources to disk
    let path1 = dir.path().join("1.thing");
    let path2 = dir.path().join("2.thing");
    let thing1 = Thing(1);
    let thing2 = Thing(2);
    std::fs::write(&path1, mr_bundle::encode(&thing1)?)?;
    std::fs::write(&path2, mr_bundle::encode(&thing2)?)?;

    // Create a Manifest that references these local resources
    let location1 = Location::Path(path1);
    let location2 = Location::Path(path2);
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

    // Pull all the pieces together into a Bundle, and assert that the same
    // resources are available
    let bundle = Bundle::<_, Thing>::from_manifest(manifest).await.unwrap();
    assert_eq!(bundle.resource(&location1), Some(&thing1));
    assert_eq!(bundle.resource(&location2), Some(&thing2));

    // Ensure that the bundle is serializable and writable
    let bundle_path = dir.path().join("test.bundle");
    std::fs::write(bundle_path, &mr_bundle::encode(&bundle)?)?;

    let decoded_bundle: Bundle<_, _> = mr_bundle::decode(&std::fs::read(bundle_path)?)?;
    assert_eq!(bundle, decoded_bundle);

    Ok(())
}
