use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::QueryInput;
use holochain_zome_types::QueryOutput;
use std::sync::Arc;

pub async fn query(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: QueryInput,
) -> RibosomeResult<QueryOutput> {
    unimplemented!();
}
