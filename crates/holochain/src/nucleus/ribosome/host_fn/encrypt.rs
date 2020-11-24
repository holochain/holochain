use crate::nucleus::ribosome::error::RibosomeResult;
use crate::nucleus::ribosome::CallContext;
use crate::nucleus::ribosome::RibosomeT;
use holochain_zome_types::EncryptInput;
use holochain_zome_types::EncryptOutput;
use std::sync::Arc;

pub fn encrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: EncryptInput,
) -> RibosomeResult<EncryptOutput> {
    unimplemented!();
}
