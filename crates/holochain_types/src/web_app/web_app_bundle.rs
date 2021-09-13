use super::WebAppManifest;
use crate::prelude::*;

/// A bundle of an AppManifest and collection of DNAs
#[derive(Debug, Serialize, Deserialize, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct WebAppBundle(mr_bundle::Bundle<WebAppManifest>);
