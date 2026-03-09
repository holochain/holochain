use crate::core::metrics::emit_signal_metric;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_types::signal::Signal;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn emit_signal(
    ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: AppSignal,
) -> Result<(), RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            non_determinism: Permission::Allow,
            ..
        } => {
            let cell_id = CellId::new(
                ribosome.dna_def_hashed().as_hash().clone(),
                call_context
                    .host_context
                    .workspace()
                    .source_chain()
                    .as_ref()
                    .expect("Must have a source chain to emit signals")
                    .agent_pubkey()
                    .clone(),
            );
            let signal = Signal::App {
                cell_id: cell_id.clone(),
                zome_name: call_context.zome.zome_name().clone(),
                signal: input,
            };
            let result = call_context
                .host_context()
                .signal_tx()
                .send(signal)
                // Only possible error here is a `SendError` which is expected if no clients are
                // connected and listening.
                .ok();

            // Record emitted signal
            if result.is_some() {
                emit_signal_metric().add(
                    1,
                    &[
                        opentelemetry::KeyValue::new("cell_id", cell_id.to_string()),
                        opentelemetry::KeyValue::new(
                            "zome",
                            call_context.zome.zome_name().to_string(),
                        ),
                    ],
                );
            }

            Ok(())
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "emit_signal".into()
            )
            .to_string()
        ))
        .into()),
    }
}
