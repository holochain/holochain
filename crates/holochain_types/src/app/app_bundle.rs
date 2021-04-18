use std::{collections::HashMap, path::PathBuf, sync::Arc};

use self::error::AppBundleResult;

use super::{dna_gamut::DnaGamut, AppManifest, AppManifestValidated};
use crate::prelude::*;

#[allow(missing_docs)]
mod error;
pub use error::*;
use futures::future::join_all;

#[cfg(test)]
mod tests;

/// A bundle of an AppManifest and collection of DNAs
#[derive(Debug, Serialize, Deserialize, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct AppBundle(mr_bundle::Bundle<AppManifest>);

impl AppBundle {
    /// Create an AppBundle from a manifest and DNA files
    pub async fn new<R: IntoIterator<Item = (PathBuf, DnaBundle)>>(
        manifest: AppManifest,
        resources: R,
        root_dir: PathBuf,
    ) -> AppBundleResult<Self> {
        let resources = join_all(resources.into_iter().map(|(path, dna_bundle)| async move {
            dna_bundle.encode().map(|bytes| (path, bytes))
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        Ok(mr_bundle::Bundle::new(manifest, resources, root_dir)?.into())
    }

    /// Construct from raw bytes
    pub fn decode(bytes: &[u8]) -> AppBundleResult<Self> {
        mr_bundle::Bundle::decode(bytes)
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Convert to the inner Bundle
    pub fn into_inner(self) -> mr_bundle::Bundle<AppManifest> {
        self.0
    }

    /// Given a DnaGamut, decide which of the available DNAs or Cells should be
    /// used for each cell in this app.
    pub async fn resolve_cells(
        self,
        agent: AgentPubKey,
        _gamut: DnaGamut,
        membrane_proofs: HashMap<SlotId, MembraneProof>,
    ) -> AppBundleResult<CellSlotResolution> {
        let AppManifestValidated { name: _, slots } = self.manifest().clone().validate()?;
        let bundle = Arc::new(self);
        let tasks = slots.into_iter().map(|(slot_id, slot)| async {
            let bundle = bundle.clone();
            Ok((slot_id, bundle.resolve_cell(slot).await?))
        });
        let resolution = futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<AppBundleResult<Vec<_>>>()?
            .into_iter()
            .fold(
                Ok(CellSlotResolution::new(agent.clone())),
                |acc: AppBundleResult<CellSlotResolution>, (slot_id, op)| {
                    if let Ok(mut resolution) = acc {
                        match op {
                            CellProvisioningOp::Create(dna, clone_limit) => {
                                let agent = resolution.agent.clone();
                                let dna_hash = dna.dna_hash().clone();
                                let cell_id = CellId::new(dna_hash, agent);
                                let slot = AppSlot::new(cell_id, true, clone_limit);
                                // TODO: could sequentialize this to remove the clone
                                let proof = membrane_proofs.get(&slot_id).cloned();
                                resolution.dnas_to_register.push((dna, proof));
                                resolution.slots.push((slot_id, slot));
                            }
                            CellProvisioningOp::Existing(cell_id, clone_limit) => {
                                let slot = AppSlot::new(cell_id, true, clone_limit);
                                resolution.slots.push((slot_id, slot));
                            }
                            CellProvisioningOp::Noop(cell_id, clone_limit) => {
                                resolution
                                    .slots
                                    .push((slot_id, AppSlot::new(cell_id, false, clone_limit)));
                            }
                            other => {
                                tracing::error!(
                                    "Encountered unexpected CellProvisioningOp: {:?}",
                                    other
                                );
                                unimplemented!()
                            }
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
        slot: AppSlotManifestValidated,
    ) -> AppBundleResult<CellProvisioningOp> {
        Ok(match slot {
            AppSlotManifestValidated::Create {
                location,
                version,
                clone_limit,
                properties,
                uid,
                deferred: _,
            } => {
                self.resolve_cell_create(&location, version.as_ref(), clone_limit, uid, properties)
                    .await?
            }

            AppSlotManifestValidated::CreateClone { .. } => {
                unimplemented!("`create_clone` provisioning strategy is currently unimplemented")
            }
            AppSlotManifestValidated::UseExisting {
                version,
                clone_limit,
                deferred: _,
            } => self.resolve_cell_existing(&version, clone_limit),
            AppSlotManifestValidated::CreateIfNotExists {
                location,
                version,
                clone_limit,
                properties,
                uid,
                deferred: _,
            } => match self.resolve_cell_existing(&version, clone_limit) {
                op @ CellProvisioningOp::Existing(_, _) => op,
                CellProvisioningOp::NoMatch => {
                    self.resolve_cell_create(
                        &location,
                        Some(&version),
                        clone_limit,
                        uid,
                        properties,
                    )
                    .await?
                }
                CellProvisioningOp::Conflict(_) => {
                    unimplemented!("conflicts are not handled, or even possible yet")
                }
                CellProvisioningOp::Create(_, _) => {
                    unreachable!("resolve_cell_existing will never return a Create op")
                }
                CellProvisioningOp::Noop(_, _) => {
                    unreachable!("resolve_cell_existing will never return a Noop")
                }
            },
            AppSlotManifestValidated::Disabled {
                version: _,
                clone_limit: _,
            } => {
                unimplemented!("`disabled` provisioning strategy is currently unimplemented")
                // CellProvisioningOp::Noop(clone_limit)
            }
        })
    }

    async fn resolve_cell_create(
        &self,
        location: &mr_bundle::Location,
        version: Option<&DnaVersionSpec>,
        clone_limit: u32,
        uid: Option<Uid>,
        properties: Option<YamlProperties>,
    ) -> AppBundleResult<CellProvisioningOp> {
        let bytes = self.resolve(location).await?;
        let dna_bundle: DnaBundle = mr_bundle::Bundle::decode(&bytes)?.into();
        let (dna_file, original_dna_hash) = dna_bundle.into_dna_file(uid, properties).await?;
        if let Some(spec) = version {
            if !spec.matches(original_dna_hash) {
                return Ok(CellProvisioningOp::NoMatch);
            }
        }
        Ok(CellProvisioningOp::Create(dna_file, clone_limit))
    }

    fn resolve_cell_existing(
        &self,
        _version: &DnaVersionSpec,
        _clone_limit: u32,
    ) -> CellProvisioningOp {
        unimplemented!("Reusing existing cells is not yet implemented")
    }
}

/// The result of running Cell resolution
// TODO: rework, make fields private
#[allow(missing_docs)]
#[derive(PartialEq, Eq, Debug)]
pub struct CellSlotResolution {
    pub agent: AgentPubKey,
    pub dnas_to_register: Vec<(DnaFile, Option<MembraneProof>)>,
    pub slots: Vec<(SlotId, AppSlot)>,
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

/// Specifies what step should be taken to provision a cell while installing an App
#[warn(missing_docs)]
#[derive(Debug)]
pub enum CellProvisioningOp {
    /// Create a new Cell
    Create(DnaFile, u32),
    /// Use an existing Cell
    Existing(CellId, u32),
    /// No provisioning needed, but there might be a clone_limit, and so we need
    /// to know which DNA and Agent to use for making clones
    Noop(CellId, u32),
    /// Couldn't find a DNA that matches the version spec; can't provision (should this be an Err?)
    NoMatch,
    /// Ambiguous result, needs manual resolution; can't provision (should this be an Err?)
    Conflict(CellProvisioningConflict),
}

/// Uninhabitable placeholder
#[derive(Debug)]
pub enum CellProvisioningConflict {}
