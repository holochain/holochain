use std::{collections::HashMap, path::PathBuf, sync::Arc};

use self::error::AppBundleResult;

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
        agent: AgentPubKey,
        membrane_proofs: HashMap<RoleName, MembraneProof>,
    ) -> AppBundleResult<AppRoleResolution> {
        let AppManifestValidated { name: _, roles } = self.manifest().clone().validate()?;
        let bundle = Arc::new(self);
        let tasks = roles.into_iter().map(|(role_name, role)| async {
            let bundle = bundle.clone();
            Ok((role_name, bundle.resolve_cell(dna_store, role).await?))
        });
        let resolution = futures::future::join_all(tasks)
            .await
            .into_iter()
            .collect::<AppBundleResult<Vec<_>>>()?
            .into_iter()
            .fold(
                Ok(AppRoleResolution::new(agent.clone())),
                |acc: AppBundleResult<AppRoleResolution>, (role_name, op)| {
                    if let Ok(mut resolution) = acc {
                        match op {
                            CellProvisioningOp::CreateFromDnaFile(dna, clone_limit) => {
                                let agent = resolution.agent.clone();
                                let dna_hash = dna.dna_hash().clone();
                                let cell_id = CellId::new(dna_hash, agent);
                                let role = AppRoleAssignment::new(cell_id, true, clone_limit);
                                // TODO: could sequentialize this to remove the clone
                                let proof = membrane_proofs.get(&role_name).cloned();
                                resolution.dnas_to_register.push((dna, proof));
                                resolution.role_assignments.push((role_name, role));
                            }

                            CellProvisioningOp::Existing(cell_id, clone_limit) => {
                                let role = AppRoleAssignment::new(cell_id, true, clone_limit);
                                resolution.role_assignments.push((role_name, role));
                            }
                            CellProvisioningOp::Noop(cell_id, clone_limit) => {
                                resolution.role_assignments.push((
                                    role_name,
                                    AppRoleAssignment::new(cell_id, false, clone_limit),
                                ));
                            }
                            other @ CellProvisioningOp::HashMismatch(_, _)
                            | other @ CellProvisioningOp::Conflict(_) => {
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
        dna_store: &impl DnaStore,
        role: AppRoleManifestValidated,
    ) -> AppBundleResult<CellProvisioningOp> {
        Ok(match role {
            AppRoleManifestValidated::Create {
                location,
                installed_hash,
                clone_limit,
                modifiers,
                deferred: _,
            } => {
                self.resolve_cell_create(
                    dna_store,
                    &location,
                    installed_hash.as_ref(),
                    clone_limit,
                    modifiers,
                )
                .await?
            }

            AppRoleManifestValidated::CreateClone { .. } => {
                unimplemented!("`create_clone` provisioning strategy is currently unimplemented")
            }
            AppRoleManifestValidated::UseExisting {
                installed_hash,
                clone_limit,
                deferred: _,
            } => self.resolve_cell_existing(&installed_hash, clone_limit),
            AppRoleManifestValidated::CreateIfNotExists {
                location,
                installed_hash,
                clone_limit,
                modifiers,
                deferred: _,
            } => match self.resolve_cell_existing(&installed_hash, clone_limit) {
                op @ CellProvisioningOp::Existing(_, _) => op,
                CellProvisioningOp::HashMismatch(_, _) => {
                    self.resolve_cell_create(
                        dna_store,
                        &location,
                        Some(&installed_hash),
                        clone_limit,
                        modifiers,
                    )
                    .await?
                }
                CellProvisioningOp::Conflict(_) => {
                    unimplemented!("conflicts are not handled, or even possible yet")
                }
                CellProvisioningOp::CreateFromDnaFile(_, _) => {
                    unreachable!("resolve_cell_existing will never return a Create op")
                }
                CellProvisioningOp::Noop(_, _) => {
                    unreachable!("resolve_cell_existing will never return a Noop")
                }
            },
            AppRoleManifestValidated::Disabled {
                installed_hash: _,
                clone_limit: _,
            } => {
                unimplemented!("`disabled` provisioning strategy is currently unimplemented")
                // CellProvisioningOp::Noop(clone_limit)
            }
        })
    }

    async fn resolve_cell_create(
        &self,
        dna_store: &impl DnaStore,
        location: &mr_bundle::Location,
        installed_hash: Option<&DnaHashB64>,
        clone_limit: u32,
        modifiers: DnaModifiersOpt,
    ) -> AppBundleResult<CellProvisioningOp> {
        let dna_file = if let Some(hash) = installed_hash {
            let (dna_file, original_hash) =
                if let Some(mut dna_file) = dna_store.get_dna(&hash.clone().into()) {
                    let original_hash = dna_file.dna_hash().clone();
                    dna_file = dna_file.update_modifiers(modifiers);
                    (dna_file, original_hash)
                } else {
                    self.resolve_location(location, modifiers).await?
                };
            let expected_hash = hash.clone().into();
            if expected_hash != original_hash {
                return Ok(CellProvisioningOp::HashMismatch(
                    expected_hash,
                    original_hash,
                ));
            }
            dna_file
        } else {
            self.resolve_location(location, modifiers).await?.0
        };
        Ok(CellProvisioningOp::CreateFromDnaFile(dna_file, clone_limit))
    }

    fn resolve_cell_existing(
        &self,
        _version: &DnaHashB64,
        _clone_limit: u32,
    ) -> CellProvisioningOp {
        unimplemented!("Reusing existing cells is not yet implemented")
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

/// This function is called in places where it will be necessary to rework that
/// area after use_existing has been implemented
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
    pub role_assignments: Vec<(RoleName, AppRoleAssignment)>,
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
    /// Create a new Cell from the given DNA file
    CreateFromDnaFile(DnaFile, u32),
    /// Use an existing Cell
    Existing(CellId, u32),
    /// No provisioning needed, but there might be a clone_limit, and so we need
    /// to know which DNA and Agent to use for making clones
    Noop(CellId, u32),
    /// The specified installed_hash does not match the actual hash of the DNA selected for provisioning. Expected: {0}, Actual: {1}
    HashMismatch(DnaHash, DnaHash),
    /// Ambiguous result, needs manual resolution; can't provision (should this be an Err?)
    Conflict(CellProvisioningConflict),
}

/// Uninhabitable placeholder
#[derive(Debug)]
pub enum CellProvisioningConflict {}
