use std::sync::Arc;

use self::error::AppBundleResult;

use super::{dna_gamut::DnaGamut, AppManifest, AppManifestValidated};
use crate::prelude::*;

#[allow(missing_docs)]
mod error;
pub use error::*;

/// A bundle of an AppManifest and collection of DNAs
#[derive(Debug, Serialize, Deserialize, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct AppBundle(mr_bundle::Bundle<AppManifest>);

impl AppBundle {
    /// Given a DnaGamut, decide which of the available DNAs or Cells should be
    /// used for each cell in this app.
    pub async fn resolve_cells(
        self,
        gamut: DnaGamut,
    ) -> AppBundleResult<Vec<(CellNick, CellProvisioningOp)>> {
        let AppManifestValidated { name: _, cells } = self.manifest().clone().validate()?;
        let bundle = Arc::new(self);
        let tasks = cells.into_iter().map(|(cell_nick, cell)| async {
            let bundle = bundle.clone();
            Ok(bundle.resolve_cell(cell).await?.map(|op| (cell_nick, op)))
        });
        Ok(futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<AppBundleResult<Vec<_>>>()?
            .into_iter()
            // Remove the `None` items
            .flatten()
            .collect())
    }

    async fn resolve_cell(
        &self,
        cell: CellManifestValidated,
    ) -> AppBundleResult<Option<CellProvisioningOp>> {
        Ok(match cell {
            CellManifestValidated::Create {
                location, version, ..
            } => Some(
                self.resolve_cell_create(&location, version.as_ref())
                    .await?,
            ),
            CellManifestValidated::CreateClone { .. } => {
                unimplemented!("`create_clone` provisioning strategy is currently unimplemented")
            }
            CellManifestValidated::UseExisting { version, .. } => {
                Some(self.resolve_cell_existing(&version))
            }
            CellManifestValidated::CreateIfNotExists {
                location, version, ..
            } => match self.resolve_cell_existing(&version) {
                CellProvisioningOp::NoMatch => {
                    Some(self.resolve_cell_create(&location, Some(&version)).await?)
                }
                op @ CellProvisioningOp::Existing(_) => Some(op),
                CellProvisioningOp::Conflict(_) => {
                    unimplemented!("conflicts are not handled, or even possible yet")
                }
                CellProvisioningOp::Create(_) => {
                    unreachable!("resolve_cell_existing will never return a Create op")
                }
            },
            CellManifestValidated::Disabled { .. } => None,
        })
    }

    async fn resolve_cell_create(
        &self,
        location: &mr_bundle::Location,
        version: Option<&DnaVersionSpec>,
    ) -> AppBundleResult<CellProvisioningOp> {
        let bytes = self.resolve(location).await?;
        let dna_bundle: DnaBundle = mr_bundle::Bundle::decode(&bytes)?.into();
        let dna_file = dna_bundle.into_dna_file().await?;
        if let Some(spec) = version {
            if !spec.matches(dna_file.dna_hash().clone()) {
                return Ok(CellProvisioningOp::NoMatch);
            }
        }
        Ok(CellProvisioningOp::Create(dna_file))
    }

    fn resolve_cell_existing(&self, _version: &DnaVersionSpec) -> CellProvisioningOp {
        unimplemented!("Reusing existing cells is not yet implemented")
    }
}

#[warn(missing_docs)]
/// Specifies what step should be taken to provision a cell while installing an App
pub enum CellProvisioningOp {
    /// Create a new Cell
    Create(DnaFile),
    /// Use an existing Cell
    Existing(CellId),
    /// Couldn't find a DNA that matches the version spec; can't provision (should this be an Err?)
    NoMatch,
    /// Ambiguous result, needs manual resolution; can't provision (should this be an Err?)
    Conflict(CellProvisioningConflict),
}

/// Uninhabitable placeholder
pub enum CellProvisioningConflict {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::prelude::*;
    use ::fixt::prelude::*;
    use app_manifest_v1::tests::app_manifest_fixture;

    use super::AppBundle;

    async fn app_bundle_fixture() -> AppBundle {
        todo!();
        // let path1 = PathBuf::from("1");
        // let path2 = PathBuf::from("2");
        // let dna1 = fixt!(DnaDef);
        // let dna2 = fixt!(DnaDef);

        // let (manifest, dna_hashes) =
        //     app_manifest_fixture(todo!("better fixture generator (sweet)"), vec![dna1, dna2]).await;

        // let resources = vec![(path1, dna1), (path2, dna2)];
    }

    #[tokio::test]
    async fn provisioning_1() {}
}
