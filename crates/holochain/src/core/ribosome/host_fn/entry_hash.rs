use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holo_hash::HasHash;
use holochain_zome_types::Entry;
use holochain_zome_types::EntryHashInput;
use holochain_zome_types::EntryHashOutput;
use std::sync::Arc;

pub fn entry_hash(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: EntryHashInput,
) -> RibosomeResult<EntryHashOutput> {
    let entry: Entry = input.into_inner();

    let entry_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
        holochain_types::entry::EntryHashed::from_content(entry).await
    })
    .into_hash();

    Ok(EntryHashOutput::new(entry_hash))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::*;
    use crate::core::ribosome::host_fn::entry_hash::entry_hash;
    use crate::core::{
        state::workspace::Workspace, workflow::unsafe_call_zome_workspace::CallZomeWorkspaceFactory,
    };
    use crate::fixt::CallContextFixturator;
    use crate::fixt::EntryFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holo_hash::EntryHash;
    use holochain_state::env::ReadManager;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::EntryHashInput;
    use holochain_zome_types::EntryHashOutput;
    use std::convert::TryInto;
    use std::sync::Arc;
    use test_wasm_common::TestString;

    #[tokio::test(threaded_scheduler)]
    /// we can get an entry hash out of the fn directly
    async fn entry_hash_test() {
        let ribosome = WasmRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let call_context = CallContextFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        let entry = EntryFixturator::new(fixt::Predictable).next().unwrap();
        let input = EntryHashInput::new(entry);

        let output: EntryHashOutput =
            entry_hash(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!(output.into_inner().get_raw().to_vec().len(), 36,);
    }

    #[tokio::test(threaded_scheduler)]
    /// we can get an entry hash out of the fn via. a wasm call
    async fn ribosome_entry_hash_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let entry = EntryFixturator::new(fixt::Predictable).next().unwrap();
        let input = EntryHashInput::new(entry);
        let mut host_access = fixt!(ZomeCallHostAccess);
        let factory: CallZomeWorkspaceFactory = env.clone().into();
        host_access.workspace = factory.clone();

        let output: EntryHashOutput =
            crate::call_test_ribosome!(host_access, TestWasm::Imports, "entry_hash", input);
        assert_eq!(output.into_inner().get_raw().to_vec().len(), 36,);
    }

    #[tokio::test(threaded_scheduler)]
    /// the hash path underlying anchors wraps entry_hash
    async fn ribosome_hash_path_pwd_test() {
        let env = holochain_state::test_utils::test_cell_env();
        let mut host_access = fixt!(ZomeCallHostAccess);
        let factory: CallZomeWorkspaceFactory = env.clone().into();
        host_access.workspace = factory.clone();

        let input = TestString::from("foo.bar".to_string());
        let output: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::HashPath, "hash", input);

        let expected_path = hdk3::hash_path::path::Path::from("foo.bar");

        let expected_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            holochain_types::entry::EntryHashed::from_content(Entry::App(
                (&expected_path).try_into().unwrap(),
            ))
            .await
        })
        .into_hash();

        assert_eq!(expected_hash.into_inner(), output.into_inner(),);
    }
}
