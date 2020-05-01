use super::HostContext;
use super::WasmRibosome;
use holochain_zome_types::CommitEntryInput;
use holochain_zome_types::CommitEntryOutput;
use std::sync::Arc;

pub async fn commit_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CommitEntryInput,
) -> CommitEntryOutput {
    unimplemented!();
}
