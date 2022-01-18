use std::sync::Arc;

use holo_hash::{DnaHash, HeaderHash};
use holochain_types::prelude::{HostFnAccess, Permission};
use holochain_util::tokio_helper;
use holochain_wasmer_host::prelude::WasmError;
use holochain_zome_types::{builder, ChainTopOrdering};

use crate::core::ribosome::{error::RibosomeError, CallContext, RibosomeT};

pub fn open_chain(
    _: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    prev_dna_hash: DnaHash,
) -> Result<HeaderHash, WasmError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            write_workspace: Permission::Allow,
            ..
        } => {
            let header_builder = builder::OpenChain { prev_dna_hash };
            let header_hash = tokio_helper::block_forever_on(tokio::task::spawn(async move {
                // push the header into the source chain
                let header_hash = call_context
                    .host_context
                    .workspace_write()
                    .source_chain()
                    .as_ref()
                    .expect("Must have source chain if write_workspace access is given")
                    .put(
                        Some(call_context.zome.clone()),
                        header_builder,
                        None,
                        ChainTopOrdering::default(),
                    )
                    .await?;
                Ok::<HeaderHash, RibosomeError>(header_hash)
            }))
            .map_err(|join_error| WasmError::Host(join_error.to_string()))?
            .map_err(|ribosome_error| WasmError::Host(ribosome_error.to_string()))?;

            Ok(header_hash)
        }
        _ => Err(WasmError::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "open_chain".into(),
            )
            .to_string(),
        )),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use holo_hash::DnaHash;
    use holochain_state::prelude::fresh_reader_test;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::InlineZome;

    use crate::sweettest::*;

    fn zome() -> InlineZome {
        InlineZome::new_unique(vec![]).callback("open_chain", move |api, prev_dna_hash: DnaHash| {
            let hash = api.open_chain(prev_dna_hash)?;
            Ok(hash)
        })
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn can_open_source_chain() {
        observability::test_run().ok();
        let mut conductor = SweetConductor::from_standard_config().await;

        let (dna1, _) = SweetDnaFile::unique_from_inline_zome("zome1", zome())
            .await
            .unwrap();

        let (dna2, _) = SweetDnaFile::unique_from_inline_zome("zome1", zome())
            .await
            .unwrap();

        let (dna3, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create])
            .await
            .unwrap();

        let apps = conductor.setup_app("app", &[dna2.clone()]).await.unwrap();
        let (cell,) = apps.into_tuple();

        // Call the "create" zome fn on Alice's app
        let hash: HeaderHash = conductor
            .call(&cell.zome("zome1"), "open_chain", dna1.dna_hash().clone())
            .await;

        let result: HeaderHash = fresh_reader_test(cell.authored_env().clone(), |txn| {
            txn.query_row("SELECT hash, MAX(seq) FROM Header", [], |row| row.get(0))
                .unwrap()
        });

        assert_eq!(result, hash);

        let apps = conductor.setup_app("app2", &[dna3.clone()]).await.unwrap();
        let (cell,) = apps.into_tuple();

        let hash: HeaderHash = conductor
            .call(
                &cell.zome(TestWasm::Create),
                "open_chain",
                dna2.dna_hash().clone(),
            )
            .await;

        let result: HeaderHash = fresh_reader_test(cell.authored_env().clone(), |txn| {
            txn.query_row("SELECT hash, MAX(seq) FROM Header", [], |row| row.get(0))
                .unwrap()
        });

        assert_eq!(result, hash);
    }
}
