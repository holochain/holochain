use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use ring::rand::SecureRandom;
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
        } => {
            // TODO - once we actually implement this in lair,
            //        generate a new random seed in lair rather
            //        than just treating the key_ref as the
            //        seed itself as we're doing here
            match input {
                Some(key_ref) => {
                    // this is a temp requirement until we do the
                    // actual lair integration
                    if key_ref.len() != 32 {
                        return Err(wasm_error!(WasmErrorInner::Host("TempErrKeyRefLen".into())).into());
                    }
                    Ok(key_ref)
                }
                None => {
                    let system_random = ring::rand::SystemRandom::new();
                    let mut key_ref = [0; 32];
                    system_random
                        .fill(&mut key_ref)
                        .map_err(|ring_unspecified| {
                            wasm_error!(WasmErrorInner::Host(ring_unspecified.to_string()))
                        })?;
                    Ok(key_ref.into())
                }
            }
        }
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
