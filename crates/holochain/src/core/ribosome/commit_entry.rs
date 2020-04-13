use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::CommitEntryInput;
use sx_zome_types::CommitEntryOutput;

pub fn commit_entry(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: CommitEntryInput,
) -> Result<CommitEntryOutput, RibosomeError> {
    unimplemented!();
}
