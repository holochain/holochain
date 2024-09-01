#![cfg(feature = "wasmer_sys")]

use crate::error::HcBundleError;
use holochain_util::ffs;
use holochain_wasmer_host::module::build_ios_module;
use mr_bundle::{Bundle, Manifest};
use std::path::Path;
use tracing::info;

pub(super) async fn build_preserialized_wasm<M: Manifest>(
    target_path: &Path,
    bundle: &Bundle<M>,
) -> Result<(), HcBundleError> {
    let target_path_folder = target_path
        .parent()
        .expect("target_path should have a parent folder");
    let _write_serialized_result =
        futures::future::join_all(bundle.bundled_resources().iter().map(
            |(relative_path, bytes)| async move {
                // only pre-serialize wasm resources
                if relative_path.extension() == Some(std::ffi::OsStr::new("wasm")) {
                    let ios_folder_path = target_path_folder.join("ios");
                    let mut resource_path_adjoined = ios_folder_path.join(
                        relative_path
                            .file_name()
                            .expect("wasm resource should have a filename"),
                    );
                    // see this code for rationale
                    // https://github.com/wasmerio/wasmer/blob/447c2e3a152438db67be9ef649327fabcad6f5b8/lib/engine-dylib/src/artifact.rs#L722-L756
                    resource_path_adjoined.set_extension("dylib");
                    ffs::create_dir_all(ios_folder_path).await?;
                    ffs::write(&resource_path_adjoined, vec![].as_slice()).await?;
                    let resource_path = ffs::canonicalize(resource_path_adjoined).await?;
                    match build_ios_module(bytes.as_slice()) {
                        Ok(module) => match module.serialize_to_file(resource_path.clone()) {
                            Ok(()) => {
                                info!("wrote ios dylib to {:?}", resource_path);
                                Ok(())
                            }
                            Err(e) => Err(HcBundleError::SerializedModuleError(e)),
                        },
                        Err(e) => Err(HcBundleError::ModuleCompileError(e)),
                    }
                } else {
                    Ok(())
                }
            },
        ))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}
