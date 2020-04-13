use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::RibosomeError;
use std::sync::Arc;
use sx_zome_types::QueryInput;
use sx_zome_types::QueryOutput;

pub fn query(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: QueryInput,
) -> Result<QueryOutput, RibosomeError> {
    unimplemented!();
}
