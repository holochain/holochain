use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::RemoteCallInput;
use holochain_zome_types::RemoteCallOutput;
use std::sync::Arc;

pub fn remote_call(
    _ribosome: Arc<impl RibosomeT>,
    _host_context: Arc<CallContext>,
    _input: RemoteCallInput,
) -> RibosomeResult<RemoteCallOutput> {
    unimplemented!();
}
