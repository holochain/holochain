#![forbid(missing_docs)]

//! Defines the CLI commands for packing/unpacking both DNA and hApp bundles

use crate::error::{HcBundleError, HcBundleResult};
use holochain_util::ffs;
use mr_bundle::{Bundle, Manifest};
use std::path::Path;
use std::path::PathBuf;

/// Unpack a DNA bundle into a working directory, returning the directory path used.
pub async fn unpack<M: Manifest>(
    extension: &'static str,
    bundle_path: &std::path::Path,
    target_dir: Option<PathBuf>,
    force: bool,
) -> HcBundleResult<PathBuf> {
    let bundle_path = ffs::canonicalize(bundle_path).await?;
    let bundle: Bundle<M> = Bundle::read_from_file(&bundle_path).await?;

    let target_dir = if let Some(d) = target_dir {
        d
    } else {
        bundle_path_to_dir(&bundle_path, extension)?
    };

    bundle.unpack_yaml(&target_dir, force).await?;

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

/// Pack a directory containing a DNA manifest into a DnaBundle, returning
/// the path to which the bundle file was written
pub async fn pack<M: Manifest>(
    dir_path: &std::path::Path,
    target_path: Option<PathBuf>,
    name: String,
) -> HcBundleResult<(PathBuf, Bundle<M>)> {
    let dir_path = ffs::canonicalize(dir_path).await?;
    let manifest_path = dir_path.join(&M::path());
    let bundle: Bundle<M> = Bundle::pack_yaml(&manifest_path).await?;
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
    bundle.write_to_file(&target_path).await?;
    Ok((target_path, bundle))
}

fn dir_to_bundle_path(dir_path: &Path, name: String, extension: &str) -> HcBundleResult<PathBuf> {
    Ok(dir_path.join(format!("{}.{}", name, extension)))
}

#[cfg(test)]
mod tests {
    use holochain_types::prelude::ValidatedDnaManifest;
    use mr_bundle::error::{MrBundleError, UnpackingError};

    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_roundtrip() {
        let tmpdir = tempfile::Builder::new()
            .prefix("hc-bundle-test")
            .tempdir()
            .unwrap();
        let dir = tmpdir.path().join("test-dna");
        std::fs::create_dir(&dir).unwrap();

        let manifest_yaml = r#"
---
manifest_version: "1"
name: test_dna
uid: blablabla
origin_time: 2022-02-11T23:29:00.789576Z
properties:
  some: 42
  props: yay
zomes:
  - name: zome1
    bundled: zome-1.wasm
  - name: zome2
    bundled: nested/zome-2.wasm
  - name: zome3
    path: ../zome-3.wasm
        "#;

        // Create files in working directory
        std::fs::create_dir(dir.join("nested")).unwrap();
        std::fs::write(dir.join("zome-1.wasm"), &[1, 2, 3]).unwrap();
        std::fs::write(dir.join("nested/zome-2.wasm"), &[4, 5, 6]).unwrap();
        std::fs::write(dir.join("dna.yaml"), manifest_yaml.as_bytes()).unwrap();

        // Create a local file that's not actually part of the bundle,
        // in the parent directory
        std::fs::write(tmpdir.path().join("zome-3.wasm"), &[7, 8, 9]).unwrap();

        let (bundle_path, bundle) =
            pack::<ValidatedDnaManifest>(&dir, None, "test_dna".to_string())
                .await
                .unwrap();

        // Ensure the bundle path was generated as expected
        assert!(bundle_path.is_file());
        assert_eq!(bundle_path, dir.join("test_dna.dna"));

        // Ensure we can resolve all files, including the local one
        assert_eq!(bundle.resolve_all().await.unwrap().values().len(), 3);

        // Unpack without forcing, which will fail
        matches::assert_matches!(
            unpack::<ValidatedDnaManifest>(
                "dna",
                &bundle_path,
                Some(bundle_path.parent().unwrap().to_path_buf()),
                false
            )
            .await,
            Err(HcBundleError::MrBundleError(MrBundleError::UnpackingError(
                UnpackingError::DirectoryExists(_)
            ),),)
        );
        // Now unpack with forcing to overwrite original directory
        unpack::<ValidatedDnaManifest>(
            "dna",
            &bundle_path,
            Some(bundle_path.parent().unwrap().to_path_buf()),
            true,
        )
        .await
        .unwrap();

        let (bundle_path, bundle) = pack::<ValidatedDnaManifest>(
            &dir,
            Some(dir.parent().unwrap().to_path_buf()),
            "test_dna".to_string(),
        )
        .await
        .unwrap();

        // Now remove the directory altogether, unpack again, and check that
        // all of the same files are present
        std::fs::remove_dir_all(&dir).unwrap();
        unpack::<ValidatedDnaManifest>("dna", &bundle_path, Some(dir.to_owned()), false)
            .await
            .unwrap();
        assert!(dir.join("zome-1.wasm").is_file());
        assert!(dir.join("nested/zome-2.wasm").is_file());
        assert!(dir.join("dna.yaml").is_file());

        // Ensure that these are the only 3 files
        assert_eq!(dir.read_dir().unwrap().collect::<Vec<_>>().len(), 3);

        // Ensure that we get the same bundle after the roundtrip
        let (_, bundle2) = pack(&dir, None, "test_dna".to_string()).await.unwrap();
        assert_eq!(bundle, bundle2);
    }
}
