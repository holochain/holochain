use super::*;
use crate::conductor::handle::ConductorHandleT;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::RibosomeT;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_custom;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_full;
use crate::core::workflow::app_validation_workflow::validation_package::get_as_author_sub_chain;
use holochain_cascade::Cascade;
use holochain_p2p::HolochainP2pDna;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_zome_types::ActionHashed;

#[instrument(skip(
    action_hashed,
    authored_db,
    dht_db,
    dht_db_cache,
    cache,
    ribosome,
    conductor_handle,
    network
))]
#[allow(clippy::too_many_arguments)]
pub(super) async fn get_as_author(
    action_hashed: ActionHashed,
    authored_db: DbRead<DbKindAuthored>,
    dht_db: DbRead<DbKindDht>,
    dht_db_cache: DhtDbQueryCache,
    cache: DbWrite<DbKindCache>,
    ribosome: &impl RibosomeT,
    conductor_handle: &dyn ConductorHandleT,
    network: &HolochainP2pDna,
) -> CellResult<ValidationPackageResponse> {
    let action = action_hashed.as_content();

    // Get the source chain with public data only
    // TODO: evaluate if we even need to use a source chain here
    // vs directly querying the database.
    let mut source_chain = SourceChainRead::new(
        authored_db.clone(),
        dht_db.clone(),
        dht_db_cache.clone(),
        conductor_handle.keystore().clone(),
        action.author().clone(),
    )
    .await?;
    source_chain.public_only();

    // Get the action data
    let (app_entry_type, action_seq) = match action
        .entry_type()
        .cloned()
        .map(|et| (et, action.action_seq()))
    {
        Some((EntryType::App(aet), action_seq)) => (aet, action_seq),
        _ => return Ok(None.into()),
    };

    // Get the required validation package
    // FIXME: Remove this completely.
    let required_validation_type = RequiredValidationType::default();

    // Gather the package
    match required_validation_type {
        RequiredValidationType::Commit => {
            // TODO: I'm not sure if we should handle this case, it seems like they should already have the commit
            Ok(None.into())
        }
        RequiredValidationType::SubChain => Ok(Some(
            get_as_author_sub_chain(action_seq, app_entry_type, &source_chain).await?,
        )
        .into()),
        RequiredValidationType::Full => {
            Ok(Some(get_as_author_full(action_seq, &source_chain).await?).into())
        }
        RequiredValidationType::Custom => {
            let cascade = Cascade::empty().with_authored(authored_db.clone());

            if let Some(commits) = cascade.get_validation_package_local(action_hashed.as_hash())? {
                return Ok(Some(ValidationPackage::new(commits)).into());
            }

            let workspace_lock = HostFnWorkspace::new(
                authored_db.clone(),
                dht_db,
                dht_db_cache,
                cache,
                conductor_handle.keystore().clone(),
                Some(action.author().clone()),
                Arc::new(ribosome.dna_def().as_content().clone()),
            )
            .await?;
            let result =
                match get_as_author_custom(&action_hashed, ribosome, network, workspace_lock)? {
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
                        ?action
                    );
                    Ok(None.into())
                }
                ValidationPackageResult::UnresolvedDependencies(deps) => {
                    info!(
                        msg = "Unresolved dependencies for custom validation package",
                        missing_dependencies = ?deps,
                        ?action
                    );
                    Ok(None.into())
                }
                ValidationPackageResult::NotImplemented => {
                    error!(
                        msg = "Entry definition specifies a custom validation package but the callback isn't defined",
                        ?action
                    );
                    Ok(None.into())
                }
            }
        }
    }
}

pub(super) async fn get_as_authority(
    action: ActionHashed,
    env: DbRead<DbKindDht>,
) -> CellResult<ValidationPackageResponse> {
    // Get author and hash
    let (action, action_hash) = action.into_inner();

    // Get the action data
    let (app_entry_type, action_seq) = match action
        .entry_type()
        .cloned()
        .map(|et| (et, action.action_seq()))
    {
        Some((EntryType::App(aet), action_seq)) => (aet, action_seq),
        _ => return Ok(None.into()),
    };

    // Get the required validation package
    // FIXME: Remove this completely.
    let required_validation_type = RequiredValidationType::default();

    let cascade = Cascade::empty().with_dht(env);

    // Gather the package
    match required_validation_type {
        RequiredValidationType::Commit => {
            // TODO: I'm not sure if we should handle this case, it seems like they should already have the commit
            Ok(None.into())
        }
        RequiredValidationType::SubChain => {
            let query = ChainQueryFilter::default()
                .include_entries(true)
                .entry_type(EntryType::App(app_entry_type))
                .sequence_range(ChainQueryFilterRange::ActionSeqRange(
                    0,
                    action_seq.saturating_sub(1),
                ));

            // Collect and return the sub chain
            let commits = match cascade.get_validation_package_local(&action_hash)? {
                Some(commits) => commits,
                None => return Ok(None.into()),
            };

            Ok(Some(ValidationPackage::new(query.filter_commits(commits))).into())
        }
        RequiredValidationType::Full => {
            let query = &ChainQueryFilter::default()
                .include_entries(true)
                .sequence_range(ChainQueryFilterRange::ActionSeqRange(
                    0,
                    action_seq.saturating_sub(1),
                ));

            // Collect and return the sub chain
            let commits = match cascade.get_validation_package_local(&action_hash)? {
                Some(commits) => commits,
                None => return Ok(None.into()),
            };

            Ok(Some(ValidationPackage::new(query.filter_commits(commits))).into())
        }
        RequiredValidationType::Custom => {
            let commits = match cascade.get_validation_package_local(&action_hash)? {
                Some(commits) => commits,
                None => return Ok(None.into()),
            };

            Ok(Some(ValidationPackage::new(commits)).into())
        }
    }
}
