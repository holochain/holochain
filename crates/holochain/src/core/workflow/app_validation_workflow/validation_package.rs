use holochain_p2p::HolochainP2pCell;
use holochain_types::HeaderHashed;
use holochain_zome_types::{
    header::AppEntryType, header::EntryType, query::ChainQueryFilter, validate::ValidationPackage,
};

use crate::core::{
    ribosome::error::RibosomeResult,
    ribosome::guest_callback::validation_package::ValidationPackageHostAccess,
    ribosome::guest_callback::validation_package::ValidationPackageInvocation,
    ribosome::guest_callback::validation_package::ValidationPackageResult, ribosome::RibosomeT,
    state::source_chain::SourceChain, workflow::CallZomeWorkspaceLock, SourceChainResult,
};
use tracing::*;

pub fn get_as_author_sub_chain(
    header_seq: u32,
    app_entry_type: AppEntryType,
    source_chain: &SourceChain,
) -> SourceChainResult<ValidationPackage> {
    // Collect and return the sub chain
    let elements = source_chain.query(
        &ChainQueryFilter::default()
            .include_entries(true)
            .entry_type(EntryType::App(app_entry_type))
            .sequence_range(0..header_seq),
    )?;
    Ok(ValidationPackage::new(elements))
}

pub fn get_as_author_full(
    header_seq: u32,
    source_chain: &SourceChain,
) -> SourceChainResult<ValidationPackage> {
    let elements = source_chain.query(
        &ChainQueryFilter::default()
            .include_entries(true)
            .sequence_range(0..header_seq),
    )?;
    Ok(ValidationPackage::new(elements))
}

pub fn get_as_author_custom(
    header_hashed: &HeaderHashed,
    ribosome: &impl RibosomeT,
    network: &HolochainP2pCell,
    workspace_lock: CallZomeWorkspaceLock,
) -> RibosomeResult<Option<ValidationPackageResult>> {
    let header = header_hashed.as_content();
    let access = ValidationPackageHostAccess::new(workspace_lock, network.clone());
    let app_entry_type = match header.entry_type() {
        Some(EntryType::App(a)) => a.clone(),
        _ => return Ok(None),
    };

    let zome_name = match ribosome
        .dna_file()
        .dna()
        .zomes
        .get(app_entry_type.zome_id().index())
    {
        Some(zome_name) => zome_name.0.clone(),
        None => {
            warn!(
                msg = "Tried to get custom validation package for header with invalid zome_id",
                ?header
            );
            return Ok(None);
        }
    };

    let invocation = ValidationPackageInvocation::new(zome_name, app_entry_type);

    Ok(Some(ribosome.run_validation_package(access, invocation)?))
}
