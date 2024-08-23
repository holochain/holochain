//! An App Bundle is an AppManifest bundled together with DNA bundles.

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use super::{AppManifest, AppManifestValidated};
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
            dna_bundle.encode().map(|bytes| (path, bytes.into()))
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

    /// Look up every installed_hash of every role, getting the DnaFiles from the DnaStore
    pub fn get_all_dnas_from_store(&self, dna_store: &impl DnaStore) -> HashMap<DnaHash, DnaFile> {
        self.manifest()
            .app_roles()
            .iter()
            .flat_map(|role| role.dna.installed_hash.to_owned())
            .map(Into::into)
            .flat_map(|hash| dna_store.get_dna(&hash).map(|dna| (hash, dna)))
            .collect()
    }

    /// Given a partial list of already available DnaFiles, fetch the missing others via
    /// mr_bundle::Location resolution
    pub async fn resolve_cells(
        self,
        dna_store: &impl DnaStore,
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
                    .resolve_cell(dna_store, role_name, role, &existing_cells)
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
        dna_store: &impl DnaStore,
        role_name: RoleName,
        role: AppRoleManifestValidated,
        existing_cells: &ExistingCellsMap,
    ) -> AppBundleResult<CellProvisioningOp> {
        match role {
            AppRoleManifestValidated::Create {
                location,
                installed_hash,
                clone_limit,
                modifiers,
                deferred: _,
            } => {
                let dna = self
                    .resolve_dna(
                        role_name,
                        dna_store,
                        &location,
                        installed_hash.as_ref(),
                        modifiers,
                    )
                    .await?;
                Ok(CellProvisioningOp::CreateFromDnaFile(dna, clone_limit))
            }

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
                location,
                modifiers,
                installed_hash,
            } => {
                let dna = self
                    .resolve_dna(
                        role_name,
                        dna_store,
                        &location,
                        installed_hash.as_ref(),
                        modifiers,
                    )
                    .await?;
                Ok(CellProvisioningOp::ProvisionOnly(dna, clone_limit))
            }
        }
    }

    async fn resolve_dna(
        &self,
        role_name: RoleName,
        dna_store: &impl DnaStore,
        location: &mr_bundle::Location,
        expected_hash: Option<&DnaHashB64>,
        modifiers: DnaModifiersOpt,
    ) -> AppBundleResult<DnaFile> {
        let dna_file = if let Some(expected_hash) = expected_hash {
            let expected_hash = expected_hash.clone().into();
            let (dna_file, original_hash) =
                if let Some(mut dna_file) = dna_store.get_dna(&expected_hash) {
                    let original_hash = dna_file.dna_hash().clone();
                    dna_file = dna_file.update_modifiers(modifiers);
                    (dna_file, original_hash)
                } else {
                    self.resolve_location(location, modifiers).await?
                };
            if expected_hash != original_hash {
                return Err(AppBundleError::CellResolutionFailure(
                    role_name,
                    format!("Hash mismatch: {} != {}", expected_hash, original_hash),
                ));
            }
            dna_file
        } else {
            self.resolve_location(location, modifiers).await?.0
        };
        Ok(dna_file)
    }

    async fn resolve_location(
        &self,
        location: &mr_bundle::Location,
        modifiers: DnaModifiersOpt,
    ) -> AppBundleResult<(DnaFile, DnaHash)> {
        let bytes = self.resolve(location).await?;
        let dna_bundle: DnaBundle = mr_bundle::Bundle::decode(&bytes)?.into();
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
