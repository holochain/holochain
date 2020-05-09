use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::guest_callback::validate::ValidateInvocation;
use crate::core::ribosome::host_fn::HostContext;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::RibosomeT;
use holo_hash::holo_hash_core::HeaderHash;
use holochain_zome_types::commit::CommitEntryResult;
use holochain_zome_types::entry::Entry;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;
use crate::core::ribosome::guest_callback::validate::ValidateResult;

pub async fn commit_entry(
    ribosome: Arc<WasmRibosome>,
    host_context: Arc<HostContext>,
    input: CommitEntryInput,
) -> RibosomeResult<CommitEntryOutput> {
    let entry: Entry = input.into_inner();
    let validate = ribosome.run_validate(ValidateInvocation {
        zome_name: host_context.zome_name.clone(),
        entry: Arc::new(entry),
    })?;
    Ok(CommitEntryOutput::new(match validate {
        // @todo move validation to a workflow
        ValidateResult::Valid => CommitEntryResult::Success(HeaderHash::new(vec![0xdb; 36])),
        invalid => CommitEntryResult::Fail,
    }))
}
