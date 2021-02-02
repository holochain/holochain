use super::{dna_gamut::DnaGamut, AppManifest};
use crate::prelude::*;

#[allow(missing_docs)]
pub mod error;

/// A bundle of an AppManifest and collection of DNAs
#[derive(Debug, Serialize, Deserialize, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct AppBundle(mr_bundle::Bundle<AppManifest>);

impl AppBundle {
    /// Given a DnaGamut, decide which of the available DNAs should be used for
    /// each cell in this app.
    pub fn resolve_dnas(&self, gamut: DnaGamut) -> Vec<DnaFile> {
        todo!()
    }
}
