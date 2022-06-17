use super::*;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use std::sync::Arc;

pub fn x_salsa20_poly1305_shared_secret_create_random(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: Option<XSalsa20Poly1305KeyRef>,
) -> Result<XSalsa20Poly1305KeyRef, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => tokio_helper::block_forever_on(async move {
            let key_ref = match input {
                Some(key_ref) => key_ref,
                None => rand_utf8::rand_utf8(
                    &mut rand::thread_rng(),
                    DEF_REF_SIZE,
                ).as_bytes().to_vec().into(),
            };

            let tag = key_ref.to_tag();

            call_context
                .host_context
                .keystore()
                .new_shared_secret(tag)
                .await?;

            holochain_keystore::LairResult::Ok(key_ref)
        })
        .map_err(|keystore_error| wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into()),
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "x_salsa20_poly1305_shared_secret_create_random".into(),
            )
            .to_string(),
        )).into()),
    }
}

// Tests for the shared secret round trip are in xsalsa20_poly1305_encrypt.
