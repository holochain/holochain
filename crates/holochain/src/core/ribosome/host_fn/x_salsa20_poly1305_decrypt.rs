use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;
use xsalsa20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use xsalsa20poly1305::XSalsa20Poly1305;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use crate::core::ribosome::HostFnAccess;

pub fn x_salsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: XSalsa20Poly1305Decrypt,
) -> Result<Option<XSalsa20Poly1305Data>, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ keystore: Permission::Allow, .. } => {
            // @todo use a libsodium wrapper instead of an ad-hoc rust implementation.
            // Note that the we're mapping any decrypting error to None here.
            // @todo this decrypt should be in lair and key refs should be refs to keys in lair
            let lib_key = GenericArray::from_slice(input.as_key_ref_ref().as_ref());
            let cipher = XSalsa20Poly1305::new(lib_key);
            let lib_nonce = GenericArray::from_slice(input.as_encrypted_data_ref().as_nonce_ref().as_ref());
            Ok(
                match cipher.decrypt(lib_nonce, input.as_encrypted_data_ref().as_encrypted_data_ref()) {
                    Ok(data) => Some(XSalsa20Poly1305Data::from(data)),
                    Err(_) => None,
                }
            )
        },
        _ => unreachable!(),
    }
}

// Tests for the decrypt round trip are in xsalsa20_poly1305_encrypt.
