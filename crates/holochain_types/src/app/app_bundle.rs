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
        membrane_proofs: HashMap<AppRoleId, MembraneProof>,
    ) -> AppBundleResult<AppRoleResolution> {
        let AppManifestValidated { name: _, roles } = self.manifest().clone().validate()?;
        let bundle = Arc::new(self);
        let tasks = roles.into_iter().map(|(role_id, role)| async {
            let bundle = bundle.clone();
            Ok((role_id, bundle.resolve_cell(role).await?))
        });
        let resolution = futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<AppBundleResult<Vec<_>>>()?
            .into_iter()
            .fold(
                Ok(AppRoleResolution::new(agent.clone())),
                |acc: AppBundleResult<AppRoleResolution>, (role_id, op)| {
                    if let Ok(mut resolution) = acc {
                        match op {
                            CellProvisioningOp::Create(dna, clone_limit) => {
                                let agent = resolution.agent.clone();
                                let dna_hash = dna.dna_hash().clone();
                                let cell_id = CellId::new(dna_hash, agent);
                                let role = AppRoleAssignment::new(cell_id, true, clone_limit);
                                // TODO: could sequentialize this to remove the clone
                                let proof = membrane_proofs.get(&role_id).cloned();
                                resolution.dnas_to_register.push((dna, proof));
                                resolution.role_assignments.push((role_id, role));
                            }
                            CellProvisioningOp::Existing(cell_id, clone_limit) => {
                                let role = AppRoleAssignment::new(cell_id, true, clone_limit);
                                resolution.role_assignments.push((role_id, role));
                            }
                            CellProvisioningOp::Noop(cell_id, clone_limit) => {
                                resolution.role_assignments.push((
                                    role_id,
                                    AppRoleAssignment::new(cell_id, false, clone_limit),
                                ));
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
        role: AppRoleManifestValidated,
    ) -> AppBundleResult<CellProvisioningOp> {
        Ok(match role {
            AppRoleManifestValidated::Create {
                location,
                version,
                clone_limit,
                modifiers,
                deferred: _,
            } => {
                self.resolve_cell_create(&location, version.as_ref(), clone_limit, modifiers)
                    .await?
            }

            AppRoleManifestValidated::CreateClone { .. } => {
                unimplemented!("`create_clone` provisioning strategy is currently unimplemented")
            }
            AppRoleManifestValidated::UseExisting {
                version,
                clone_limit,
                deferred: _,
            } => self.resolve_cell_existing(&version, clone_limit),
            AppRoleManifestValidated::CreateIfNotExists {
                location,
                version,
                clone_limit,
                modifiers,
                deferred: _,
            } => match self.resolve_cell_existing(&version, clone_limit) {
                op @ CellProvisioningOp::Existing(_, _) => op,
                CellProvisioningOp::NoMatch => {
                    self.resolve_cell_create(&location, Some(&version), clone_limit, modifiers)
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
            AppRoleManifestValidated::Disabled {
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
        modifiers: DnaModifiersOpt,
    ) -> AppBundleResult<CellProvisioningOp> {
        let bytes = self.resolve(location).await?;
        let dna_bundle: DnaBundle = mr_bundle::Bundle::decode(&bytes)?.into();
        let (dna_file, original_dna_hash) = dna_bundle.into_dna_file(modifiers).await?;
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

/// This function is called in places where it will be necessary to rework that
/// area after use_existing has been implemented
#[deprecated = "Raising visibility into a change that needs to happen after `use_existing` is implemented"]
pub fn we_must_remember_to_rework_cell_panic_handling_after_implementing_use_existing_cell_resolution(
) {
}

/// The answer to the question:
/// "how do we concretely assign DNAs to the open roles of this App?"
/// Includes the DNAs selected to fill the roles and the details of the role assignments.
// TODO: rework, make fields private
#[allow(missing_docs)]
#[derive(PartialEq, Eq, Debug)]
pub struct AppRoleResolution {
    pub agent: AgentPubKey,
    pub dnas_to_register: Vec<(DnaFile, Option<MembraneProof>)>,
    pub role_assignments: Vec<(AppRoleId, AppRoleAssignment)>,
}

#[allow(missing_docs)]
impl AppRoleResolution {
    pub fn new(agent: AgentPubKey) -> Self {
        Self {
            agent,
            dnas_to_register: Default::default(),
            role_assignments: Default::default(),
        }
    }

    /// Return the IDs of new cells to be created as part of the resolution.
    /// Does not return existing cells to be reused.
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
