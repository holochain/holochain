use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::HostContext;
use holo_hash::Hashable;
use holo_hash::Hashed;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryHashInput;
use holochain_zome_types::EntryHashOutput;
use std::sync::Arc;

pub fn entry_hash(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    input: EntryHashInput,
) -> RibosomeResult<EntryHashOutput> {
    let entry: Entry = input.into_inner();

    let entry_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        holochain_types::entry::EntryHashed::with_data(entry).await
    })?
    .into_hash();

    let core_hash: holo_hash_core::HoloHashCore = entry_hash.into();

    Ok(EntryHashOutput::new(core_hash))
}

#[cfg(test)]
pub mod wasm_test {
    use crate::core::ribosome::host_fn::entry_hash::entry_hash;
    use crate::core::ribosome::HostContextFixturator;
    use crate::fixt::EntryFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use holo_hash_core::HoloHashCoreHash;
    // use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::EntryHashInput;
    use holochain_zome_types::EntryHashOutput;
    use std::sync::Arc;
    // use holochain_state::env::ReadManager;
    // use crate::core::state::workspace::Workspace;

    #[tokio::test(threaded_scheduler)]
    /// we can get an entry hash out of the fn directly
    async fn entry_hash_test() {
        let ribosome = WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let host_context = HostContextFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let entry = EntryFixturator::new(fixt::Predictable).next().unwrap();
        let input = EntryHashInput::new(entry);

        let output: EntryHashOutput =
            entry_hash(Arc::new(ribosome), Arc::new(host_context), input).unwrap();

        assert_eq!(
            vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 153, 246, 31, 194
            ],
            output.into_inner().get_raw().to_vec()
        );
    }

    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // /// we can get an entry hash out of the fn via. a wasm call
    // async fn ribosome_entry_hash_test() {
    //     let env = holochain_state::test_utils::test_cell_env();
    //     let dbs = env.dbs().await;
    //     let env_ref = env.guard().await;
    //     let reader = env_ref.reader().unwrap();
    //     let mut workspace = crate::core::workflow::InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
    //
    //     let entry = EntryFixturator::new(fixt::Predictable).next().unwrap();
    //     let input = EntryHashInput::new(entry);
    //     let output: EntryHashOutput =
    //         crate::call_test_ribosome!(workspace, TestWasm::Imports, "entry_hash", input);
    //     assert_eq!(
    //         vec![
    //             0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //             0, 0, 0, 0, 153, 246, 31, 194
    //         ],
    //         output.into_inner().get_raw().to_vec()
    //     );
    // }
}
