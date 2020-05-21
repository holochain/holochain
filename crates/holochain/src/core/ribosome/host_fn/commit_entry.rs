use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::guest_callback::validate::ValidateResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::RibosomeT;
use holo_hash::holo_hash_core::HeaderHash;
use holochain_zome_types::commit::CommitEntryResult;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;

pub async fn commit_entry(
    ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    input: CommitEntryInput,
) -> RibosomeResult<CommitEntryOutput> {
    let entry: Entry = input.into_inner();
    let validate = ribosome.run_validate(
        host_context.workspace.clone(),
        ValidateInvocation {
            zome_name: host_context.zome_name(),
            entry: Arc::new(entry),
        },
    )?;
    Ok(CommitEntryOutput::new(match validate {
        // @todo move validation to a workflow
        // the only reason this is here is so we can write realistic looking tests prior
        // to having the full workflow driven callbacks implemented
        ValidateResult::Valid => CommitEntryResult::Success(HeaderHash::new(vec![0xdb; 36])),
        invalid => CommitEntryResult::Fail(format!("{:?}", invalid)),
    }))
}
