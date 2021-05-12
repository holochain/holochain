use super::*;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_custom;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_full;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_sub_chain;
use holochain_cascade::Cascade;
use holochain_p2p::HolochainP2pCell;
use holochain_state::source_chain::SourceChain;
use holochain_types::dna::DnaFile;
use holochain_zome_types::HeaderHashed;

#[instrument(skip(header_hashed, env, cache, ribosome, conductor_api, network))]
pub(super) async fn get_as_author(
    header_hashed: HeaderHashed,
    env: EnvRead,
    cache: EnvWrite,
    ribosome: &impl RibosomeT,
    conductor_api: &impl CellConductorApiT,
    network: &HolochainP2pCell,
) -> CellResult<ValidationPackageResponse> {
    let header = header_hashed.as_content();

    // Get the source chain with public data only
    // TODO: evaluate if we even need to use a source chain here
    // vs directly querying the database.
    let mut source_chain = SourceChain::new(env.clone(), header.author().clone())?;
    source_chain.public_only();

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
            let cascade = Cascade::empty().with_vault(env.clone());

            if let Some(elements) =
                cascade.get_validation_package_local(&header_hashed.as_hash())?
            {
                return Ok(Some(ValidationPackage::new(elements)).into());
            }

            let workspace_lock = HostFnWorkspace::new(
                env.into(),
                cache,
                conductor_api.cell_id().agent_pubkey().clone(),
            )?;
            let result =
                match get_as_author_custom(&header_hashed, ribosome, network, workspace_lock)? {
                    Some(result) => result,
                    None => return Ok(None.into()),
                };
            match result {
                ValidationPackageResult::Success(validation_package) => {
                    // TODO: Cache the package for future calls

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

    let cascade = Cascade::empty().with_vault(env.clone());

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
