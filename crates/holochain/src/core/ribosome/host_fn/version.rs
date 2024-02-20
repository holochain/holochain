use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::version::ZomeApiVersion;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn version(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<ZomeApiVersion, RuntimeError> {
    unreachable!();
}
