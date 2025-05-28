//! Defines the CLI commands for packing/unpacking DNA, hApp, and web-hApp bundles.

use crate::error::{HcBundleError, HcBundleResult};
use holochain_util::ffs;
use mr_bundle::{FileSystemBundler, Manifest};
use std::path::Path;
use std::path::PathBuf;

/// Expand an existing bundle into a working directory, returning the directory path used.
pub async fn expand_bundle<M: Manifest>(
    bundle_path: &Path,
    target_dir: Option<PathBuf>,
    force: bool,
) -> HcBundleResult<PathBuf> {
    let bundle_path = ffs::canonicalize(bundle_path).await?;
    let bundle = FileSystemBundler::load_from::<M>(&bundle_path).await?;

    let target_dir = if let Some(d) = target_dir {
        d
    } else {
        bundle_path_to_dir(&bundle_path, M::bundle_extension())?
    };

    FileSystemBundler::expand_to(&bundle, &target_dir, force).await?;

    Ok(target_dir)
}

/// Unpack a bundle into a working directory, returning the directory path used.
pub async fn expand_unknown_bundle(
    bundle_path: &Path,
    extension: &'static str,
    manifest_file_name: &'static str,
    target_dir: Option<PathBuf>,
    force: bool,
) -> HcBundleResult<PathBuf> {
    let bundle_path = ffs::canonicalize(bundle_path).await?;
    let bundle = FileSystemBundler::load_from::<serde_yaml::Value>(&bundle_path).await?;

    let target_dir = if let Some(d) = target_dir {
        d
    } else {
        bundle_path_to_dir(&bundle_path, extension)?
    };

    FileSystemBundler::expand_named_to(&bundle, manifest_file_name, &target_dir, force).await?;

    Ok(target_dir)
}

fn bundle_path_to_dir(path: &Path, extension: &'static str) -> HcBundleResult<PathBuf> {
    let bad_ext_err = || HcBundleError::FileExtensionMissing(extension, path.to_owned());
    let ext = path.extension().ok_or_else(bad_ext_err)?;
    if ext != extension {
        return Err(bad_ext_err());
    }
    let stem = path
        .file_stem()
        .expect("A file with an extension also has a stem");

    Ok(path
        .parent()
        .expect("file path should have parent")
        .join(stem))
}

/// Pack a directory containing a YAML manifest (DNA, hApp, Web hApp) into a bundle, returning
/// the path to which the bundle file was written.
pub async fn pack<M: Manifest>(
    dir_path: &Path,
    target_path: Option<PathBuf>,
    name: String,
) -> HcBundleResult<PathBuf> {
    let dir_path = ffs::canonicalize(dir_path).await?;
    let manifest_path = dir_path.join(M::file_name());

    let target_path = match target_path {
        Some(target_path) => {
            if target_path.is_dir() {
                dir_to_bundle_path(&target_path, name, M::bundle_extension())?
            } else {
                target_path
            }
        }
        None => dir_to_bundle_path(&dir_path, name, M::bundle_extension())?,
    };

    FileSystemBundler::bundle_to::<M>(&manifest_path, &target_path).await?;

    Ok(target_path)
}

fn dir_to_bundle_path(dir_path: &Path, name: String, extension: &str) -> HcBundleResult<PathBuf> {
    Ok(dir_path.join(format!("{}.{}", name, extension)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use holochain_types::prelude::ValidatedDnaManifest;
    use mr_bundle::error::MrBundleError;

    #[tokio::test(flavor = "multi_thread")]
    #[cfg_attr(target_os = "windows", ignore = "unc path mismatch - use dunce")]
    async fn test_round_trip() {
        let tmpdir = tempfile::Builder::new()
            .prefix("hc-bundle-test")
            .tempdir()
            .unwrap();
        let dir = tmpdir.path().join("test-dna");
        std::fs::create_dir(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();

        let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
integrity:
    network_seed: blablabla
    properties:
      some: 42
      props: yay
    zomes:
      - name: zome1
        path: zome-1.wasm
      - name: zome2
        path: nested/zome-2.wasm
      - name: zome3
        path: ../zome-3.wasm
        "#;

        // Create files in working directory
        std::fs::create_dir(dir.join("nested")).unwrap();
        std::fs::write(dir.join("zome-1.wasm"), [1, 2, 3]).unwrap();
        std::fs::write(dir.join("nested/zome-2.wasm"), [4, 5, 6]).unwrap();
        std::fs::write(dir.join("dna.yaml"), manifest_yaml.as_bytes()).unwrap();

        // Create a local file that's not actually part of the bundle,
        // in the parent directory
        std::fs::write(tmpdir.path().join("zome-3.wasm"), [7, 8, 9]).unwrap();

        let bundle_path = pack::<ValidatedDnaManifest>(&dir, None, "test_dna".to_string())
            .await
            .unwrap();
        let bundle = FileSystemBundler::load_from::<ValidatedDnaManifest>(&bundle_path)
            .await
            .unwrap();
        // Ensure the bundle path was generated as expected
        assert!(bundle_path.is_file());
        assert_eq!(bundle_path, dir.join("test_dna.dna"));

        println!("Loaded bundle: {:?}", bundle);

        // Ensure we can resolve all files, including the local one
        assert_eq!(bundle.get_all_resources().values().len(), 3);

        // Unpack without forcing, which will fail
        matches::assert_matches!(
            expand_bundle::<ValidatedDnaManifest>(
                &bundle_path,
                Some(bundle_path.parent().unwrap().to_path_buf()),
                false
            )
            .await,
            Err(HcBundleError::MrBundleError(
                MrBundleError::DirectoryExists(_)
            ))
        );
        // Now unpack with forcing to overwrite original directory
        expand_bundle::<ValidatedDnaManifest>(
            &bundle_path,
            Some(bundle_path.parent().unwrap().to_path_buf()),
            true,
        )
        .await
        .unwrap();

        let bundle_path = pack::<ValidatedDnaManifest>(
            &dir,
            Some(dir.parent().unwrap().to_path_buf()),
            "test_dna".to_string(),
        )
        .await
        .unwrap();

        // Now remove the directory altogether, unpack again, and check that
        // all the same files are present
        std::fs::remove_dir_all(&dir).unwrap();
        expand_bundle::<ValidatedDnaManifest>(&bundle_path, Some(dir.to_owned()), false)
            .await
            .unwrap();

        assert!(dir.join("zome-1.wasm").is_file());
        assert!(dir.join("zome-2.wasm").is_file());
        assert!(dir.join("zome-3.wasm").is_file());
        assert!(dir.join("dna.yaml").is_file());

        // Ensure that these are 4 files
        assert_eq!(dir.read_dir().unwrap().collect::<Vec<_>>().len(), 4);

        // Ensure that we get the same bundle after the round trip
        let bundle_path = pack::<ValidatedDnaManifest>(&dir, None, "test_dna".to_string())
            .await
            .unwrap();
        let bundle2 = FileSystemBundler::load_from::<ValidatedDnaManifest>(bundle_path)
            .await
            .unwrap();
        assert_eq!(bundle, bundle2);
    }
}
