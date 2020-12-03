use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::XSalsa20Poly1305DecryptInput;
use holochain_zome_types::XSalsa20Poly1305DecryptOutput;
use std::sync::Arc;

pub fn xsalsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: XSalsa20Poly1305DecryptInput,
) -> RibosomeResult<XSalsa20Poly1305DecryptOutput> {
    unimplemented!();
}
