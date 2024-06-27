use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::{CallContext, RibosomeT};
use holo_hash::hash_type;
use holochain_sqlite::prelude::DbRead;
use holochain_state::prelude::{validation_receipts_for_action, validation_receipts_for_entry};
use holochain_types::access::{HostFnAccess, Permission};
use holochain_util::tokio_helper;
use holochain_wasmer_host::prelude::{wasm_error, WasmError, WasmErrorInner};
use holochain_zome_types::prelude::{GetValidationReceiptsInput, ValidationReceiptSet};
use std::sync::Arc;
use wasmer::RuntimeError;
use holochain_sqlite::db::DbKindDht;

#[tracing::instrument(skip(_ribosome, call_context), fields(?call_context.zome, function = ?call_context.function_name))]
pub fn get_validation_receipts(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetValidationReceiptsInput,
) -> Result<Vec<ValidationReceiptSet>, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let results = tokio_helper::block_forever_on(async move {
                let dht_db: DbRead<DbKindDht> = call_context.host_context.workspace().databases().1;

                let hash = input.for_hash;
                match hash.hash_type() {
                    hash_type::AnyDht::Action => {
                        dht_db
                            .read_async(move |txn| {
                                validation_receipts_for_action(
                                    &txn,
                                    hash.into_action_hash()
                                        .expect("Type has been checked as action hash"),
                                )
                            })
                            .await
                    }
                    hash_type::AnyDht::Entry => {
                        dht_db
                            .read_async(move |txn| {
                                validation_receipts_for_entry(
                                    &txn,
                                    hash.into_entry_hash()
                                        .expect("Type has been checked as entry hash"),
                                )
                            })
                            .await
                    }
                }
            })
            .map_err(|e| wasm_error!(WasmErrorInner::Host(e.to_string())))?;

            Ok(results)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_validation_receipts".into(),
            )
            .to_string(),
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use crate::core::ribosome::host_fn::create::create;
    use crate::core::ribosome::host_fn::get_validation_receipts::get_validation_receipts;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use crate::fixt::{CallContextFixturator, RealRibosomeFixturator};
    use ::fixt::Predictable;
    use ::fixt::{fixt, Unpredictable};
    use holochain_wasm_test_utils::{TestWasm, TestWasmPair};
    use holochain_zome_types::prelude::*;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread")]
    async fn call_get_validation_receipts() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![TestWasm::Crd]))
            .next()
            .unwrap();
        let mut call_context = CallContextFixturator::new(Unpredictable).next().unwrap();
        call_context.zome = TestWasmPair::<IntegrityZome, CoordinatorZome>::from(TestWasm::Crd)
            .coordinator
            .erase_type();
        let host_access = fixt!(ZomeCallHostAccess, Predictable);
        call_context.host_context = host_access.into();

        let app_entry = EntryFixturator::new(AppEntry).next().unwrap();
        let input = CreateInput::new(
            EntryDefLocation::app(0, 0),
            EntryVisibility::Public,
            app_entry.clone(),
            ChainTopOrdering::default(),
        );
        let ribosome_handle = Arc::new(ribosome);
        let action_hash = create(
            ribosome_handle.clone(),
            Arc::new(call_context.clone()),
            input,
        )
        .unwrap();

        let receipts = get_validation_receipts(
            ribosome_handle.clone(),
            Arc::new(call_context.clone()),
            GetValidationReceiptsInput::for_action(action_hash),
        )
        .unwrap();

        // Not the most useful test/assertion. Just checking that this doesn't error and checking
        // that this gives back validation receipts will require an integration test.
        assert!(receipts.is_empty());

        // Try with an entry hash that doesn't exist, which exercises the code path for entry hash
        // and should return an empty list of validation receipts.
        let entry_hash = fixt!(EntryHash);
        let receipts = get_validation_receipts(
            ribosome_handle.clone(),
            Arc::new(call_context.clone()),
            GetValidationReceiptsInput::for_entry(entry_hash),
        )
        .unwrap();
        assert!(receipts.is_empty());
    }
}
