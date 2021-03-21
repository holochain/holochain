use super::*;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_custom;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_full;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_sub_chain;
use call_zome_workflow::CallZomeWorkspaceLock;
use holochain_cascade::Cascade;
use holochain_cascade::DbPair;
use holochain_cascade::DbPairMut;
use holochain_p2p::HolochainP2pCell;
use holochain_sqlite::error::DatabaseResult;
use holochain_types::dna::DnaFile;
use holochain_zome_types::HeaderHashed;

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
    pub(super) fn create(env: EnvRead) -> DatabaseResult<Self> {
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
    env: EnvRead,
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
        ribosome.dna_def(),
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
        RequiredValidationType::SubChain => Ok(Some(get_as_author_sub_chain(
            header_seq,
            app_entry_type,
            &source_chain,
        )?)
        .into()),
        RequiredValidationType::Full => {
            Ok(Some(get_as_author_full(header_seq, &source_chain)?).into())
        }
        RequiredValidationType::Custom => {
            let element_authored = ElementBuf::authored(env.clone(), false)?;
            let meta_authored = MetadataBuf::authored(env.clone())?;
            let mut element_cache = ElementBuf::cache(env.clone())?;
            let mut meta_cache = MetadataBuf::cache(env.clone())?;
            let cascade = Cascade::empty()
                .with_cache(DbPairMut::new(&mut element_cache, &mut meta_cache))
                .with_authored(DbPair::new(&element_authored, &meta_authored));

            if let Some(elements) =
                cascade.get_validation_package_local(&header_hashed.as_hash())?
            {
                return Ok(Some(ValidationPackage::new(elements)).into());
            }

            let workspace_lock = CallZomeWorkspaceLock::new(CallZomeWorkspace::new(env)?);
            let result =
                match get_as_author_custom(&header_hashed, ribosome, network, workspace_lock)? {
                    Some(result) => result,
                    None => return Ok(None.into()),
                };
            match result {
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
                    error!(
                        msg = "Entry definition specifies a custom validation package but the callback isn't defined",
                        ?header
                    );
                    Ok(None.into())
                }
            }
        }
    }
}

pub(super) async fn get_as_authority(
    header: HeaderHashed,
    env: EnvRead,
    dna_file: &DnaFile,
    conductor_api: &impl CellConductorApiT,
) -> CellResult<ValidationPackageResponse> {
    // Get author and hash
    let (header, header_hash) = header.into_inner();

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
        dna_file.dna(),
        conductor_api,
    )
    .await?;

    // Get the required validation package
    let required_validation_type = match entry_def {
        Some(ed) => ed.required_validation_type,
        None => return Ok(None.into()),
    };

    let element_integrated = ElementBuf::vault(env.clone(), false)?;
    let meta_integrated = MetadataBuf::vault(env.clone())?;
    let mut element_cache = ElementBuf::cache(env.clone())?;
    let mut meta_cache = MetadataBuf::cache(env.clone())?;
    let cascade = Cascade::empty()
        .with_cache(DbPairMut::new(&mut element_cache, &mut meta_cache))
        .with_integrated(DbPair::new(&element_integrated, &meta_integrated));

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
            let elements = match cascade.get_validation_package_local(&header_hash)? {
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
            let elements = match cascade.get_validation_package_local(&header_hash)? {
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
            let elements = match cascade.get_validation_package_local(&header_hash)? {
                Some(elements) => elements,
                None => return Ok(None.into()),
            };

            Ok(Some(ValidationPackage::new(elements)).into())
        }
    }
}
