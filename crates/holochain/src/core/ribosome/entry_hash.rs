use super::HostContext;
use super::WasmRibosome;
use holochain_zome_types::EntryHashInput;
use holochain_zome_types::EntryHashOutput;
use std::sync::Arc;

pub async fn entry_hash(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EntryHashInput,
) -> EntryHashOutput {
    unimplemented!();
}
