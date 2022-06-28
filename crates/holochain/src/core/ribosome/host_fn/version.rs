use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use holochain_wasmer_host::prelude::*;
use holochain_zome_types::version::ZomeApiVersion;

pub fn version(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: (),
) -> Result<ZomeApiVersion, RuntimeError> {
    unreachable!();
}
