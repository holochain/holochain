use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;

pub fn x_salsa20_poly1305_shared_secret_ingest(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: XSalsa20Poly1305SharedSecretIngest,
) -> Result<XSalsa20Poly1305KeyRef, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            keystore: Permission::Allow,
            ..
        } => {
            tokio_helper::block_forever_on(async move {
                let mut s_pk: [u8; 32] = [0; 32];
                s_pk.copy_from_slice(input.as_sender_ref().as_ref());
                let mut r_pk: [u8; 32] = [0; 32];
                r_pk.copy_from_slice(input.as_recipient_ref().as_ref());

                let edata = input.as_encrypted_data_ref();
                let mut nonce: [u8; 24] = [0; 24];
                nonce.copy_from_slice(edata.as_nonce_ref().as_ref());
                let data = edata.as_encrypted_data_ref().to_vec();

                let res = call_context
                    .host_context
                    .keystore()
                    .crypto_box_xsalsa_open(s_pk.into(), r_pk.into(), nonce, data.into())
                    .await?;

                // this is a temp requirement until we do the
                // actual lair integration
                if res.len() != 32 {
                    return Err("TempErrKeyRefLen".into());
                }

                // TODO - once we actually implement this in lair,
                //        insert this shared secret in lair
                //        and return the key_ref, rather than using
                //        the secret AS the key_ref like we're doing here

                holochain_keystore::LairResult::Ok(res.as_ref().into())
            })
            .map_err(|keystore_error| WasmError::Host(keystore_error.to_string()))
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "x_salsa20_poly1305_shared_secret_ingest".into(),
            )
            .to_string(),
        )),
    }
}
