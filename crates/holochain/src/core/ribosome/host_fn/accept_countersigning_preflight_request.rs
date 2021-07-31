use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use crate::core::sys_validate::check_countersigning_preflight_request;
use holochain_keystore::KeystoreSenderExt;
use tracing::error;

#[allow(clippy::extra_unused_lifetimes)]
pub fn accept_countersigning_preflight_request<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: PreflightRequest,
) -> Result<PreflightRequestAcceptance, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ agent_info: Permission::Allow, keystore: Permission::Allow, non_determinism: Permission::Allow, .. } => {
            if let Err(e) = check_countersigning_preflight_request(&input) {
                return Ok(PreflightRequestAcceptance::Invalid(e.to_string()));
            }
            tokio_helper::block_forever_on(async move {
                if holochain_types::timestamp::now().0 + SESSION_TIME_FUTURE_MAX_MILLIS < input.session_times().start().0
                {
                    return Ok(PreflightRequestAcceptance::UnacceptableFutureStart);
                }

                let author = call_context.host_context.workspace().source_chain().agent_pubkey().clone();

                let agent_index = match input
                    .signing_agents()
                    .iter()
                    .position(|(agent, _)| agent == &author)
                {
                    Some(agent_index) => agent_index as u8,
                    None => return Ok(PreflightRequestAcceptance::UnacceptableAgentNotFound),
                };
                let countersigning_agent_state = call_context.host_context.workspace().source_chain().accept_countersigning_preflight_request(input.clone(), agent_index).await.map_err(|source_chain_error| WasmError::Host(source_chain_error.to_string()))?;
                let signature: Signature = match call_context
                    .host_context
                    .keystore()
                    .sign(Sign::new_raw(
                        author,
                        PreflightResponse::encode_fields_for_signature(&input, &countersigning_agent_state)?,
                    ))
                    .await
                {
                    Ok(signature) => signature,
                    Err(e) => {
                        // Attempt to unlock the chain again.
                        // If this fails the chain will remain locked until the session end time.
                        // But also we're handling a keystore error already so we should return that.
                        if let Err(unlock_result) = call_context
                            .host_context
                            .workspace()
                            .source_chain()
                            .unlock_chain()
                            .await {
                                error!(?unlock_result);
                            }
                        return Err(WasmError::Host(e.to_string()));
                    }
                };

                Ok(PreflightRequestAcceptance::Accepted(PreflightResponse::new(
                    input,
                    countersigning_agent_state,
                    signature,
                )))
            })
        },
        _ => unreachable!(),
    }
}