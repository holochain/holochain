#![cfg(feature = "wasmer_wamr")]

use crate::error::HcBundleError;
use mr_bundle::{Bundle, Manifest};
use std::path::Path;

pub(super) async fn build_preserialized_wasm<M: Manifest>(
    _target_path: &Path,
    _bundle: &Bundle<M>,
) -> Result<(), HcBundleError> {
    unimplemented!("The feature flag 'wasmer_sys' must be enabled to support compiling wasm");
}
