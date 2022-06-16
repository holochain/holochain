use holochain_p2p::HolochainP2pDna;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::prelude::SourceChainRead;
use holochain_types::prelude::*;
use holochain_zome_types::ActionHashed;

use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageHostAccess;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageInvocation;
use crate::core::ribosome::guest_callback::validation_package::ValidationPackageResult;
use crate::core::ribosome::RibosomeT;
use crate::core::SourceChainResult;
use tracing::*;

pub async fn get_as_author_sub_chain(
    action_seq: u32,
    app_entry_type: AppEntryType,
    source_chain: &SourceChainRead,
) -> SourceChainResult<ValidationPackage> {
    // Collect and return the sub chain
    let elements = source_chain
        .query(
            ChainQueryFilter::default()
                .include_entries(true)
                .entry_type(EntryType::App(app_entry_type))
                .sequence_range(ChainQueryFilterRange::ActionSeqRange(
                    0,
                    action_seq.saturating_sub(1),
                )),
        )
        .await?;
    Ok(ValidationPackage::new(elements))
}

pub async fn get_as_author_full(
    action_seq: u32,
    source_chain: &SourceChainRead,
) -> SourceChainResult<ValidationPackage> {
    let elements = source_chain
        .query(
            ChainQueryFilter::default()
                .include_entries(true)
                .sequence_range(ChainQueryFilterRange::ActionSeqRange(
                    0,
                    action_seq.saturating_sub(1),
                )),
        )
        .await?;
    Ok(ValidationPackage::new(elements))
}

pub fn get_as_author_custom(
    action_hashed: &ActionHashed,
    ribosome: &impl RibosomeT,
    network: &HolochainP2pDna,
    workspace_lock: HostFnWorkspaceRead,
) -> RibosomeResult<Option<ValidationPackageResult>> {
    let action = action_hashed.as_content();
    let access = ValidationPackageHostAccess::new(workspace_lock, network.clone());
    let app_entry_type = match action.entry_type() {
        Some(EntryType::App(a)) => a.clone(),
        _ => return Ok(None),
    };

    let zome = match ribosome.find_zome_from_entry(&app_entry_type.id()) {
        Some(zome_tuple) => zome_tuple,
        None => {
            warn!(
                msg = "Tried to get custom validation package for action with invalid zome_id",
                ?action
            );
            return Ok(None);
        }
    };

    let invocation = ValidationPackageInvocation::new(zome, app_entry_type);

    Ok(Some(ribosome.run_validation_package(access, invocation)?))
}
