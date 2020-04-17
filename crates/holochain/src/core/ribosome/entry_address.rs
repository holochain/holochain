use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::EntryAddressInput;
use sx_zome_types::EntryAddressOutput;

pub fn entry_address(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: EntryAddressInput,
) -> EntryAddressOutput {
    unimplemented!();
}
