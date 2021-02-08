use std::{collections::HashMap, sync::Arc};

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
        agent: AgentPubKey,
        gamut: DnaGamut,
        membrane_proofs: HashMap<CellNick, MembraneProof>,
    ) -> AppBundleResult<CellSlotResolution> {
        let AppManifestValidated { name: _, cells } = self.manifest().clone().validate()?;
        let bundle = Arc::new(self);
        let tasks = cells.into_iter().map(|(cell_nick, cell)| async {
            let bundle = bundle.clone();
            Ok((cell_nick, bundle.resolve_cell(cell).await?))
        });
        let resolution = futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<AppBundleResult<Vec<_>>>()?
            .into_iter()
            .fold(
                Ok(CellSlotResolution::new(agent.clone())),
                |acc: AppBundleResult<CellSlotResolution>, (cell_nick, op)| {
                    if let Ok(mut resolution) = acc {
                        match op {
                            CellProvisioningOp::Create(dna, clone_limit) => {
                                let dna_hash = dna.dna_hash().clone();
                                let cell_id = CellId::new(dna_hash, agent.clone());
                                let slot = CellSlot::new(Some(cell_id.clone()), clone_limit);
                                // TODO: could sequentialize this to remove the clone
                                let proof = membrane_proofs.get(&cell_nick).cloned();
                                resolution.dnas_to_register.push((dna, proof));
                                resolution.slots.push((cell_nick, slot));
                            }
                            CellProvisioningOp::Existing(cell_id, clone_limit) => {
                                let slot = CellSlot::new(Some(cell_id.clone()), clone_limit);
                                resolution.slots.push((cell_nick, slot));
                            }
                            CellProvisioningOp::Noop(clone_limit) => {
                                resolution
                                    .slots
                                    .push((cell_nick, CellSlot::new(None, clone_limit)));
                            }
                            _ => todo!(),
                        }
                        Ok(resolution)
                    } else {
                        acc
                    }
                },
            )?;

        // let resolution = cells.into_iter();
        Ok(resolution)
    }

    async fn resolve_cell(
        &self,
        cell: CellManifestValidated,
    ) -> AppBundleResult<CellProvisioningOp> {
        Ok(match cell {
            CellManifestValidated::Create {
                location,
                version,
                clone_limit,
                ..
            } => {
                self.resolve_cell_create(&location, version.as_ref(), clone_limit)
                    .await?
            }

            CellManifestValidated::CreateClone { .. } => {
                unimplemented!("`create_clone` provisioning strategy is currently unimplemented")
            }
            CellManifestValidated::UseExisting {
                version,
                clone_limit,
                ..
            } => self.resolve_cell_existing(&version, clone_limit),
            CellManifestValidated::CreateIfNotExists {
                location,
                version,
                clone_limit,
                ..
            } => match self.resolve_cell_existing(&version, clone_limit) {
                op @ CellProvisioningOp::Existing(_, _) => op,
                CellProvisioningOp::NoMatch => {
                    self.resolve_cell_create(&location, Some(&version), clone_limit)
                        .await?
                }
                CellProvisioningOp::Conflict(_) => {
                    unimplemented!("conflicts are not handled, or even possible yet")
                }
                CellProvisioningOp::Create(_, _) => {
                    unreachable!("resolve_cell_existing will never return a Create op")
                }
                CellProvisioningOp::Noop(_) => {
                    unreachable!("resolve_cell_existing will never return a Noop")
                }
            },
            CellManifestValidated::Disabled { clone_limit } => {
                CellProvisioningOp::Noop(clone_limit)
            }
        })
    }

    async fn resolve_cell_create(
        &self,
        location: &mr_bundle::Location,
        version: Option<&DnaVersionSpec>,
        clone_limit: u32,
    ) -> AppBundleResult<CellProvisioningOp> {
        let bytes = self.resolve(location).await?;
        let dna_bundle: DnaBundle = mr_bundle::Bundle::decode(&bytes)?.into();
        let dna_file = dna_bundle.into_dna_file().await?;
        if let Some(spec) = version {
            if !spec.matches(dna_file.dna_hash().clone()) {
                return Ok(CellProvisioningOp::NoMatch);
            }
        }
        Ok(CellProvisioningOp::Create(dna_file, clone_limit))
    }

    fn resolve_cell_existing(
        &self,
        _version: &DnaVersionSpec,
        clone_limit: u32,
    ) -> CellProvisioningOp {
        unimplemented!("Reusing existing cells is not yet implemented")
    }
}

/// The result of running Cell resolution
// TODO: rework, make fields private
#[allow(missing_docs)]
pub struct CellSlotResolution {
    pub agent: AgentPubKey,
    pub dnas_to_register: Vec<(DnaFile, Option<MembraneProof>)>,
    pub slots: Vec<(CellNick, CellSlot)>,
}

#[allow(missing_docs)]
impl CellSlotResolution {
    pub fn new(agent: AgentPubKey) -> Self {
        Self {
            agent,
            dnas_to_register: Default::default(),
            slots: Default::default(),
        }
    }

    /// Return the IDs of new cells to be created as part of the resolution.
    /// Does not return existing cells to be reused.
    // TODO: remove clone of MembraneProof
    pub fn cells_to_create(&self) -> Vec<(CellId, Option<MembraneProof>)> {
        self.dnas_to_register
            .iter()
            .map(|(dna, proof)| {
                (
                    CellId::new(dna.dna_hash().clone(), self.agent.clone()),
                    proof.clone(),
                )
            })
            .collect()
    }
}

#[warn(missing_docs)]
/// Specifies what step should be taken to provision a cell while installing an App
pub enum CellProvisioningOp {
    /// Create a new Cell
    Create(DnaFile, u32),
    /// Use an existing Cell
    Existing(CellId, u32),
    /// No provisioning needed (but there might be a clone_limit)
    Noop(u32),
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
