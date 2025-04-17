use mr_bundle::{
    resource_id_for_path, Bundle, FileSystemBundler, Manifest, ResourceBytes, ResourceIdentifier,
};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "manifest_version")]
#[allow(missing_docs)]
enum TestManifest {
    #[serde(rename = "1")]
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

#[tokio::test]
async fn file_system_bundler() {
    let dir = tempfile::tempdir().unwrap();

    // Write a ResourceBytes to disk
    let resource: ResourceBytes = Thing("some content".into()).into();
    let resource_path = PathBuf::from("another/nested/bundled.thing");
    tokio::fs::create_dir_all(dir.path().join(resource_path.parent().unwrap()))
        .await
        .unwrap();
    tokio::fs::write(dir.path().join(&resource_path), resource)
        .await
        .unwrap();

    // Create a Manifest that references these resources
    let manifest = TestManifest::V1(ManifestV1 {
        name: "name".to_string(),
        things: vec![ThingManifest {
            location: resource_path.to_str().unwrap().to_string(),
        }],
    });

    // Write the manifest to disk
    let manifest_path = dir.path().join(TestManifest::file_name());
    tokio::fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap())
        .await
        .unwrap();

    // Create a Bundle from the manifest, and write it to the filesystem
    let bundle_path = dir
        .path()
        .join(format!("test-bundle.{}", TestManifest::bundle_extension()));
    FileSystemBundler::bundle_to::<TestManifest>(manifest_path, bundle_path)
        .await
        .unwrap();

    // Read the bundle back from disk
    let bundle_bytes = tokio::fs::read(
        dir.path()
            .join(format!("test-bundle.{}", TestManifest::bundle_extension())),
    )
    .await
    .unwrap();

    // Unpack the bundle
    let bundle = Bundle::<TestManifest>::unpack(bundle_bytes.as_slice()).unwrap();

    // Now try to dump the unpacked bundle to disk
    let unpacked_dir = dir.path().join("unpacked");
    FileSystemBundler::expand_to(&bundle, &unpacked_dir, false)
        .await
        .unwrap();

    // Check that the manifest and resource files were written correctly
    assert!(unpacked_dir.join(TestManifest::file_name()).is_file());
    assert!(unpacked_dir.join("bundled.thing").is_file());

    // It should still be possible to read back the bumped bundle and get the exact same thing
    let manifest_path = unpacked_dir.join(TestManifest::file_name());
    let rebundle = FileSystemBundler::bundle::<TestManifest>(manifest_path)
        .await
        .unwrap();
    let rebundle_bytes = rebundle.pack().unwrap();

    assert_eq!(bundle_bytes, rebundle_bytes);
}

#[tokio::test]
async fn file_system_bundler_with_raw_bundle() {
    let dir = tempfile::tempdir().unwrap();

    // Write a ResourceBytes to disk
    let resource: ResourceBytes = Thing("some content".into()).into();
    let resource_path = PathBuf::from("another/nested/bundled.thing");
    tokio::fs::create_dir_all(dir.path().join(resource_path.parent().unwrap()))
        .await
        .unwrap();
    tokio::fs::write(dir.path().join(&resource_path), resource)
        .await
        .unwrap();

    // Create a Manifest that references these resources
    let mut manifest = TestManifest::V1(ManifestV1 {
        name: "name".to_string(),
        things: vec![ThingManifest {
            location: resource_path.to_str().unwrap().to_string(),
        }],
    });

    // Write the manifest to disk
    let manifest_path = dir.path().join(TestManifest::file_name());
    tokio::fs::write(&manifest_path, serde_yaml::to_string(&manifest).unwrap())
        .await
        .unwrap();

    // Create a Bundle from the manifest, and write it to the filesystem
    let bundle_path = dir
        .path()
        .join(format!("test-bundle.{}", TestManifest::bundle_extension()));
    FileSystemBundler::bundle_to::<TestManifest>(manifest_path, &bundle_path)
        .await
        .unwrap();

    //
    // Now switch to working with the raw bundle
    //

    // Read the bundle back from disk
    let bundle_bytes = tokio::fs::read(
        dir.path()
            .join(format!("test-bundle.{}", TestManifest::bundle_extension())),
    )
    .await
    .unwrap();

    // Read and unpack the bundle
    let bundle = Bundle::<serde_yaml::Value>::unpack(bundle_bytes.as_slice()).unwrap();

    let unpacked_dir = dir.path().join("unpacked");
    FileSystemBundler::expand_named_to(&bundle, "unknown.yaml", &unpacked_dir, false)
        .await
        .unwrap();

    // Check that the manifest and resource files were written correctly
    assert!(unpacked_dir.join("unknown.yaml").is_file());

    // Force updating resource ids so that it would be expected to match the written content.
    manifest.generate_resource_ids();
    assert_eq!(
        serde_yaml::to_string(&manifest).unwrap(),
        std::fs::read_to_string(unpacked_dir.join("unknown.yaml")).unwrap()
    );

    assert!(unpacked_dir.join("bundled.thing").is_file());
    assert_eq!(
        "some content",
        std::fs::read_to_string(unpacked_dir.join("bundled.thing")).unwrap()
    );
}
