use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::SignInput;
use holochain_zome_types::SignOutput;
use std::sync::Arc;

pub fn sign(
    _ribosome: Arc<impl RibosomeT>,
    _host_context: Arc<CallContext>,
    _input: SignInput,
) -> RibosomeResult<SignOutput> {
    unimplemented!();
}
