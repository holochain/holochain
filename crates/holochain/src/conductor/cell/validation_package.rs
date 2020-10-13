use call_zome_workflow::CallZomeWorkspaceLock;
use holochain_p2p::HolochainP2pCell;
use holochain_state::{env::EnvironmentRead, error::DatabaseResult, prelude::*};
use holochain_types::{dna::DnaFile, HeaderHashed};

use crate::core::{
    ribosome::guest_callback::validation_package::ValidationPackageHostAccess,
    ribosome::guest_callback::validation_package::ValidationPackageInvocation,
    ribosome::guest_callback::validation_package::ValidationPackageResult,
    ribosome::RibosomeT,
    state::cascade::{Cascade, DbPair, DbPairMut},
};

use super::*;

/// Databases to search for validation package
pub(super) struct ValidationPackageDb {
    element_integrated: ElementBuf<IntegratedPrefix>,
    meta_integrated: MetadataBuf<IntegratedPrefix>,
    element_rejected: ElementBuf<RejectedPrefix>,
    meta_rejected: MetadataBuf<RejectedPrefix>,
    element_authored: ElementBuf<AuthoredPrefix>,
    meta_authored: MetadataBuf<AuthoredPrefix>,
}

impl ValidationPackageDb {
    pub(super) fn create(env: EnvironmentRead) -> DatabaseResult<Self> {
        Ok(Self {
            element_integrated: ElementBuf::vault(env.clone(), false)?,
            element_rejected: ElementBuf::rejected(env.clone())?,
            element_authored: ElementBuf::authored(env.clone(), false)?,
            meta_integrated: MetadataBuf::vault(env.clone())?,
            meta_rejected: MetadataBuf::rejected(env.clone())?,
            meta_authored: MetadataBuf::authored(env)?,
        })
    }

    pub(super) fn cascade(&self) -> Cascade {
        Cascade::empty()
            .with_integrated(DbPair::new(&self.element_integrated, &self.meta_integrated))
            .with_rejected(DbPair::new(&self.element_rejected, &self.meta_rejected))
            .with_authored(DbPair::new(&self.element_authored, &self.meta_authored))
    }
}

#[instrument(skip(header_hashed, env, ribosome, conductor_api, network))]
pub(super) async fn get_as_author(
    header_hashed: HeaderHashed,
    env: EnvironmentRead,
    ribosome: &impl RibosomeT,
    conductor_api: &impl CellConductorApiT,
    network: &HolochainP2pCell,
) -> CellResult<ValidationPackageResponse> {
    let header = header_hashed.as_content();

    // Get the source chain with public data only
    let source_chain = SourceChain::public_only(env.clone())?;

    // Get the header data
    let (app_entry_type, header_seq) = match header
        .entry_type()
        .cloned()
        .map(|et| (et, header.header_seq()))
    {
        Some((EntryType::App(aet), header_seq)) => (aet, header_seq),
        _ => return Ok(None.into()),
    };

    //Get entry def
    let entry_def = get_entry_def_from_ids(
        app_entry_type.zome_id(),
        app_entry_type.id(),
        ribosome.dna_file(),
        conductor_api,
    )
    .await?;

    // Get the required validation package
    let required_validation_type = match entry_def {
        Some(ed) => ed.required_validation_type,
        None => return Ok(None.into()),
    };

    // Gather the package
    match required_validation_type {
        RequiredValidationType::Element => {
            // TODO: I'm not sure if we should handle this case, it seems like they should already have the element
            Ok(None.into())
        }
        RequiredValidationType::SubChain => {
            // Collect and return the sub chain
            let elements = source_chain.query(
                &ChainQueryFilter::default()
                    .include_entries(true)
                    .entry_type(EntryType::App(app_entry_type))
                    .sequence_range(0..header_seq),
            )?;
            Ok(Some(ValidationPackage::new(elements)).into())
        }
        RequiredValidationType::Full => {
            let elements = source_chain.query(
                &ChainQueryFilter::default()
                    .include_entries(true)
                    .sequence_range(0..header_seq),
            )?;
            Ok(Some(ValidationPackage::new(elements)).into())
        }
        RequiredValidationType::Custom => {
            let element_authored = ElementBuf::authored(env.clone(), false)?;
            let meta_authored = MetadataBuf::authored(env.clone())?;
            let mut element_cache = ElementBuf::cache(env.clone())?;
            let mut meta_cache = MetadataBuf::cache(env.clone())?;
            let cascade = Cascade::empty()
                .with_cache(DbPairMut::new(&mut element_cache, &mut meta_cache))
                .with_authored(DbPair::new(&element_authored, &meta_authored));

            if let Some(elements) = cascade.get_validation_package_local(
                header.author().clone(),
                &header_hashed,
                required_validation_type,
            )? {
                return Ok(Some(ValidationPackage::new(elements)).into());
            }

            let workspace_lock = CallZomeWorkspaceLock::new(CallZomeWorkspace::new(env)?);
            let access = ValidationPackageHostAccess::new(workspace_lock, network.clone());
            let app_entry_type = match header.entry_type() {
                Some(EntryType::App(a)) => a.clone(),
                _ => return Ok(None.into()),
            };

            let zome_name = match ribosome
                .dna_file()
                .dna()
                .zomes
                .get(app_entry_type.zome_id().index())
            {
                Some(zome_name) => zome_name.0.clone(),
                None => {
                    warn!(msg = "Tried to get custom validation package for header with invalid zome_id", ?header);
                    return Ok(None.into());
                }
            };

            let invocation = ValidationPackageInvocation::new(zome_name, app_entry_type);

            match ribosome.run_validation_package(access, invocation)? {
                ValidationPackageResult::Success(validation_package) => {
                    // Cache the package for future calls
                    meta_cache.register_validation_package(
                        header_hashed.as_hash(),
                        validation_package
                            .0
                            .iter()
                            .map(|el| el.header_address().clone()),
                    );

                    Ok(Some(validation_package).into())
                }
                ValidationPackageResult::Fail(reason) => {
                    warn!(
                        msg = "Getting custom validation package fail",
                        error = %reason,
                        ?header
                    );
                    Ok(None.into())
                }
                ValidationPackageResult::UnresolvedDependencies(deps) => {
                    info!(
                        msg = "Unresolved dependencies for custom validation package",
                        missing_dependencies = ?deps,
                        ?header
                    );
                    Ok(None.into())
                }
                ValidationPackageResult::NotImplemented => {
                    error!(msg = "Entry definition specifies a custom validation package but the callback isn't defined", ?header);
                    Ok(None.into())
                }
            }
        }
    }
}

pub(super) async fn get_as_authority(
    header: HeaderHashed,
    env: EnvironmentRead,
    dna_file: &DnaFile,
    conductor_api: &impl CellConductorApiT,
) -> CellResult<ValidationPackageResponse> {
    // Get author and hash
    let (header, header_hash) = header.into_inner();
    let agent = header.author().clone();

    // Get the header data
    let (app_entry_type, header_seq) = match header
        .entry_type()
        .cloned()
        .map(|et| (et, header.header_seq()))
    {
        Some((EntryType::App(aet), header_seq)) => (aet, header_seq),
        _ => return Ok(None.into()),
    };

    //Get entry def
    let entry_def = get_entry_def_from_ids(
        app_entry_type.zome_id(),
        app_entry_type.id(),
        dna_file,
        conductor_api,
    )
    .await?;

    // Get the required validation package
    let required_validation_type = match entry_def {
        Some(ed) => ed.required_validation_type,
        None => return Ok(None.into()),
    };

    let mut element_cache = ElementBuf::cache(env.clone())?;
    let mut meta_cache = MetadataBuf::cache(env.clone())?;
    let cascade = Cascade::empty().with_cache(DbPairMut::new(&mut element_cache, &mut meta_cache));

    let header_hashed = HeaderHashed::with_pre_hashed(header, header_hash);

    // Gather the package
    match required_validation_type {
        RequiredValidationType::Element => {
            // TODO: I'm not sure if we should handle this case, it seems like they should already have the element
            Ok(None.into())
        }
        RequiredValidationType::SubChain => {
            let query = ChainQueryFilter::default()
                .include_entries(true)
                .entry_type(EntryType::App(app_entry_type))
                .sequence_range(0..header_seq);

            // Collect and return the sub chain
            let elements = match cascade.get_validation_package_local(
                agent,
                &header_hashed,
                required_validation_type,
            )? {
                Some(elements) => elements,
                None => return Ok(None.into()),
            };

            let elements = elements
                .into_iter()
                .filter(|el| query.check(el.header()))
                .collect();

            Ok(Some(ValidationPackage::new(elements)).into())
        }
        RequiredValidationType::Full => {
            let query = &ChainQueryFilter::default()
                .include_entries(true)
                .sequence_range(0..header_seq);

            // Collect and return the sub chain
            let elements = match cascade.get_validation_package_local(
                agent,
                &header_hashed,
                required_validation_type,
            )? {
                Some(elements) => elements,
                None => return Ok(None.into()),
            };

            let elements = elements
                .into_iter()
                .filter(|el| query.check(el.header()))
                .collect();

            Ok(Some(ValidationPackage::new(elements)).into())
        }
        RequiredValidationType::Custom => {
            let elements = match cascade.get_validation_package_local(
                agent,
                &header_hashed,
                required_validation_type,
            )? {
                Some(elements) => elements,
                None => return Ok(None.into()),
            };

            Ok(Some(ValidationPackage::new(elements)).into())
        }
    }
}
