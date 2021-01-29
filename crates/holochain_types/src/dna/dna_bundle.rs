use super::DnaManifest;

/// A bundle of Wasm zomes, respresented as a file.
#[derive(
    Debug, serde::Serialize, serde::Deserialize, shrinkwraprs::Shrinkwrap, derive_more::From,
)]
pub struct DnaBundle(mr_bundle::Bundle<DnaManifest>);
