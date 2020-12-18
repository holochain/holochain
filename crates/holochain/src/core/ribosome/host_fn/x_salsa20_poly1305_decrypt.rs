use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::x_salsa20_poly1305::data::XSalsa20Poly1305Data;
use holochain_zome_types::XSalsa20Poly1305DecryptInput;
use holochain_zome_types::XSalsa20Poly1305DecryptOutput;
use std::sync::Arc;
use xsalsa20poly1305::aead::{generic_array::GenericArray, Aead, NewAead};
use xsalsa20poly1305::XSalsa20Poly1305;

pub fn x_salsa20_poly1305_decrypt(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: XSalsa20Poly1305DecryptInput,
) -> RibosomeResult<XSalsa20Poly1305DecryptOutput> {
    let decrypt = input.into_inner();

    // @todo use a libsodium wrapper instead of an ad-hoc rust implementation.
    // Note that the we're mapping any decrypting error to None here.
    // @todo this decrypt should be in lair and key refs should be refs to keys in lair
    let lib_key = GenericArray::from_slice(decrypt.as_key_ref_ref().as_ref());
    let cipher = XSalsa20Poly1305::new(lib_key);
    let lib_nonce = GenericArray::from_slice(decrypt.as_encrypted_data_ref().as_nonce_ref().as_ref());
    let lib_data: Option<XSalsa20Poly1305Data> =
        match cipher.decrypt(lib_nonce, decrypt.as_encrypted_data_ref().as_encrypted_data_ref()) {
            Ok(data) => Some(XSalsa20Poly1305Data::from(data)),
            Err(_) => None,
        };

    Ok(XSalsa20Poly1305DecryptOutput::new(lib_data))
}

// Tests for the decrypt round trip are in xsalsa20_poly1305_encrypt.
