use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_util::tokio_helper;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn create_x25519_keypair(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    _input: (),
) -> Result<X25519PubKey, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => tokio_helper::block_forever_on(async move {
            call_context
                .host_context
                .keystore()
                .new_x25519_keypair_random()
                .await
                .map(|k| (*k).into())
        })
        .map_err(|keystore_error| {
            wasm_error!(WasmErrorInner::Host(keystore_error.to_string())).into()
        }),
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "create_x25519_keypair".into()
            )
            .to_string()
        ))
        .into()),
    }
}

// See x_25519_x_salsa20_poly1305_encrypt for testing encryption using created keypairs.
