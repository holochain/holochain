pub(super) async fn build_preserialized_wasm<M: Manifest>(
    _target_path: &PathBuf,
    _bundle: &Bundle<M>,
) -> Result<(), HcBundleError> {
    unimplemented!("The feature flag 'wasmer_sys' must be enabled to support compiling wasm");
}
