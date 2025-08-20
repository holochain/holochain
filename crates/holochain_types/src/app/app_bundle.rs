//! An App Bundle is an AppManifest bundled together with DNA bundles.

use super::{AppManifest, AppManifestValidated};
use crate::prelude::*;
use bytes::Buf;
use mr_bundle::error::MrBundleError;
use mr_bundle::{Bundle, ResourceIdentifier};
use std::io::Read;
use std::sync::Arc;

#[allow(missing_docs)]
mod error;
pub use error::*;

#[cfg(test)]
mod tests;

/// A bundle of an AppManifest and collection of DNAs
#[derive(Debug, Serialize, Deserialize, Clone, derive_more::From, shrinkwraprs::Shrinkwrap)]
pub struct AppBundle(Bundle<AppManifest>);

impl AppBundle {
    /// Create an AppBundle from a manifest and DNA files
    pub fn new<R: IntoIterator<Item = (String, DnaBundle)>>(
        manifest: AppManifest,
        resources: R,
    ) -> AppBundleResult<Self> {
        let resources = resources
            .into_iter()
            .map(|(resource_id, dna_bundle)| {
                dna_bundle.pack().map(|bytes| (resource_id, bytes.into()))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Bundle::new(manifest, resources)?.into())
    }

    /// Construct from raw bytes
    pub fn unpack(source: impl Read) -> AppBundleResult<Self> {
        Bundle::unpack(source).map(Into::into).map_err(Into::into)
    }

    /// Convert to the inner Bundle
    pub fn into_inner(self) -> Bundle<AppManifest> {
        self.0
    }

    /// Given a partial list of already available DnaFiles, fetch the missing others via
    /// mr_bundle::Location resolution
    pub async fn resolve_cells(
        self,
        membrane_proofs: MemproofMap,
        existing_cells: ExistingCellsMap,
    ) -> AppBundleResult<AppRoleResolution> {
        let AppManifestValidated { name: _, roles } = self.manifest().clone().validate()?;
        let bundle = Arc::new(self);
        let tasks = roles.into_iter().map(|(role_name, role)| async {
            let bundle = bundle.clone();
            Ok((
                role_name.clone(),
                bundle
                    .resolve_cell(role_name, role, &existing_cells)
                    .await?,
            ))
        });

        futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<AppBundleResult<Vec<_>>>()?
            .into_iter()
            .try_fold(
                AppRoleResolution::default(),
                |mut resolution: AppRoleResolution, (role_name, op)| {
                    match op {
                        CellProvisioningOp::CreateFromDnaFile(dna, clone_limit) => {
                            let dna_hash = dna.dna_hash().clone();
                            let role = AppRolePrimary::new(dna_hash, true, clone_limit).into();
                            // TODO: could sequentialize this to remove the clone
                            let proof = membrane_proofs.get(&role_name).cloned();
                            resolution.dnas_to_register.push((dna, proof));
                            resolution.role_assignments.push((role_name, role));
                        }

                        CellProvisioningOp::Existing(cell_id, protected) => {
                            let role = AppRoleDependency { cell_id, protected }.into();
                            resolution.role_assignments.push((role_name, role));
                        }

                        CellProvisioningOp::ProvisionOnly(dna, clone_limit) => {
                            let dna_hash = dna.dna_hash().clone();

                            // TODO: could sequentialize this to remove the clone
                            let proof = membrane_proofs.get(&role_name).cloned();
                            resolution.dnas_to_register.push((dna, proof));
                            resolution.role_assignments.push((
                                role_name,
                                AppRolePrimary::new(dna_hash, false, clone_limit).into(),
                            ));
                        }
                    }

                    Ok(resolution)
                },
            )
    }

    async fn resolve_cell(
        &self,
        role_name: RoleName,
        role: AppRoleManifestValidated,
        existing_cells: &ExistingCellsMap,
    ) -> AppBundleResult<CellProvisioningOp> {
        match role {
            AppRoleManifestValidated::Create {
                path: resource_id,
                clone_limit,
                modifiers,
                ..
            } => {
                let dna = self.get_modified_dna_file(&resource_id, modifiers).await?.0;
                Ok(CellProvisioningOp::CreateFromDnaFile(dna, clone_limit))
            }

            #[allow(deprecated)]
            AppRoleManifestValidated::UseExisting {
                compatible_hash,
                protected,
            } => {
                if let Some(cell_id) = existing_cells.get(&role_name) {
                    Ok(CellProvisioningOp::Existing(cell_id.clone(), protected))
                } else {
                    Err(AppBundleError::CellResolutionFailure(
                        role_name,
                        format!("No existing cell was specified for the role with DNA {compatible_hash}"),
                    ))
                }
            }

            AppRoleManifestValidated::CloneOnly {
                clone_limit,
                path: resource_id,
                modifiers,
                installed_hash: _,
            } => {
                let dna = self.get_modified_dna_file(&resource_id, modifiers).await?.0;
                Ok(CellProvisioningOp::ProvisionOnly(dna, clone_limit))
            }
        }
    }

    async fn get_modified_dna_file(
        &self,
        resource_id: &ResourceIdentifier,
        modifiers: DnaModifiersOpt,
    ) -> AppBundleResult<(DnaFile, DnaHash)> {
        let bytes: bytes::Bytes = self
            .get_resource(resource_id)
            .ok_or_else(|| MrBundleError::MissingResources(vec![resource_id.clone()]))?
            .clone()
            .into();
        let dna_bundle: DnaBundle = Bundle::unpack(bytes.reader())?.into();
        let (dna_file, original_hash) = dna_bundle.into_dna_file(modifiers).await?;
        Ok((dna_file, original_hash))
    }
}

/// The answer to the question:
/// "how do we concretely assign DNAs to the open roles of this App?"
/// Includes the DNAs selected to fill the roles and the details of the role assignments.
// TODO: rework, make fields private
#[allow(missing_docs)]
#[derive(PartialEq, Eq, Debug, Default)]
pub struct AppRoleResolution {
    pub dnas_to_register: Vec<(DnaFile, Option<MembraneProof>)>,
    pub role_assignments: Vec<(RoleName, AppRoleAssignment)>,
}

#[allow(missing_docs)]
impl AppRoleResolution {
    /// Return the IDs of new cells to be created as part of the resolution.
    /// Does not return existing cells to be reused.
    pub fn cells_to_create(&self, agent_key: AgentPubKey) -> Vec<(CellId, Option<MembraneProof>)> {
        let provisioned = self
            .role_assignments
            .iter()
            .filter_map(|(_name, role)| {
                let role = role.as_primary()?;
                if role.is_provisioned {
                    Some(CellId::new(role.dna_hash().clone(), agent_key.clone()))
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<_>>();

        self.dnas_to_register
            .iter()
            .filter_map(|(dna, proof)| {
                let cell_id = CellId::new(dna.dna_hash().clone(), agent_key.clone());
                if provisioned.contains(&cell_id) {
                    Some((cell_id, proof.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Specifies what step should be taken to provision a cell while installing an App
#[warn(missing_docs)]
#[derive(Debug)]
pub enum CellProvisioningOp {
    /// Create a new Cell from the given DNA file
    CreateFromDnaFile(DnaFile, u32),
    /// Use an existing Cell
    Existing(CellId, bool),
    /// No creation needed, but there might be a clone_limit, and so we need
    /// to know which DNA to use for making clones
    ProvisionOnly(DnaFile, u32),
}
