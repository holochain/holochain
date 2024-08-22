use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_wasmer_host::prelude::*;

use holochain_types::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn close_chain(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: CloseChainInput,
) -> Result<ActionHash, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            // Construct the close chain action
            let action_builder = builder::CloseChain::new(input.new_target);

            let action_hash = tokio_helper::block_forever_on(tokio::task::spawn(async move {
                // push the action into the source chain
                let action_hash = call_context
                    .host_context
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given")
                    .put_weightless(action_builder, None, ChainTopOrdering::Strict)
                    .await?;
                Ok::<ActionHash, RibosomeError>(action_hash)
            }))
            .map_err(|join_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(join_error.to_string())).into()
            })?
            .map_err(|ribosome_error| -> RuntimeError {
                wasm_error!(WasmErrorInner::Host(ribosome_error.to_string())).into()
            })?;

            // Return the hash of the chain close
            Ok(action_hash)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "close_chain".into()
            )
            .to_string()
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::close_chain;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::fixt::{CallContextFixturator, RealRibosomeFixturator};
    use ::fixt::Predictable;
    use ::fixt::{fixt, Unpredictable};
    use holochain_util::tokio_helper;
    use holochain_wasm_test_utils::{TestWasm, TestWasmPair};
    use holochain_zome_types::prelude::*;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn call_close_chain() {
        // Note that any zome will do here, we're not calling its functions!
        let ribosome =
            RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::MigrateInitial]))
                .next()
                .unwrap();
        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.zome =
            TestWasmPair::<IntegrityZome, CoordinatorZome>::from(TestWasm::MigrateInitial)
                .coordinator
                .erase_type();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        let mut input = CloseChainInput {
            new_target: Some(fixt!(MigrationTarget)),
        };

        // If this is an agent migration, the agent keypair needs to exist
        // so the Close can be signed.
        if let Some(MigrationTarget::Agent(agent)) = input.new_target.as_mut() {
            *agent = host_access
                .keystore
                .new_sign_keypair_random()
                .await
                .unwrap();
        }

        let host_access_2 = host_access.clone();
        call_context.host_context = host_access.into();

        let output = close_chain(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        // the chain head should be the committed chain close action
        let chain_head = tokio_helper::block_forever_on(async move {
            host_access_2
                .workspace
                .source_chain()
                .as_ref()
                .unwrap()
                .chain_head()
                .unwrap()
                .unwrap()
                .action
        });

        assert_eq!(chain_head, output);
    }
}
