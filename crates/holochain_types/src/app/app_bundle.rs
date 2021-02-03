use std::sync::Arc;

use super::{dna_gamut::DnaGamut, AppManifest, AppManifestValidated};
use crate::prelude::*;

#[allow(missing_docs)]
pub mod error;

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
                todo!("Come back to this after implementing DNA cloning")
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
                CellProvisioningOp::Conflict(_) => unimplemented!("conflicts not possible yet"),
                CellProvisioningOp::Create(_) => {
                    unreachable!("resolve_cell_existing will not return a Create op")
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
        todo!()
    }

    fn resolve_cell_existing(&self, version: &DnaVersionSpec) -> CellProvisioningOp {
        unimplemented!("Reusing existing cells is not yet implemented")
    }
}

#[warn(missing_docs)]
pub enum CellProvisioningOp {
    Create(DnaHash),
    Existing(CellId),
    NoMatch,
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

    pub async fn app_bundle_fixture() -> AppBundle {
        let path1 = PathBuf::from("1");
        let path2 = PathBuf::from("2");
        let dna1 = fixt!(DnaDef);
        let dna2 = fixt!(DnaDef);

        let (manifest, dna_hashes) =
            app_manifest_fixture(todo!("better fixture generator (sweet)"), vec![dna1, dna2]).await;

        let resources = vec![(path1, dna1), (path2, dna2)];
    }

    #[tokio::test]
    async fn provisioning_1() {}
}
