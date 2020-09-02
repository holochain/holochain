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
        holochain_types::entry::EntryHashed::from_content_sync(entry)
    })
    .into_hash();

    Ok(EntryHashOutput::new(entry_hash))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::*;
    use crate::core::ribosome::host_fn::entry_hash::entry_hash;

    use crate::fixt::CallContextFixturator;
    use crate::fixt::EntryFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use fixt::prelude::*;
    use holo_hash::EntryHash;
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

        assert_eq!(output.into_inner().get_full_bytes().to_vec().len(), 36,);
    }

    #[tokio::test(threaded_scheduler)]
    /// we can get an entry hash out of the fn via. a wasm call
    async fn ribosome_entry_hash_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let dbs = env.dbs();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into(), &dbs).unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let entry = EntryFixturator::new(fixt::Predictable).next().unwrap();
        let input = EntryHashInput::new(entry);
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        let output: EntryHashOutput =
            crate::call_test_ribosome!(host_access, TestWasm::EntryHash, "entry_hash", input);
        assert_eq!(output.into_inner().get_full_bytes().to_vec().len(), 36,);
    }

    #[tokio::test(threaded_scheduler)]
    /// the hash path underlying anchors wraps entry_hash
    async fn ribosome_hash_path_pwd_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let dbs = env.dbs();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into(), &dbs).unwrap();

        let (_g, raw_workspace) =
            crate::core::workflow::unsafe_call_zome_workspace::UnsafeCallZomeWorkspace::from_mut(
                &mut workspace,
            );

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = raw_workspace;
        let input = TestString::from("foo.bar".to_string());
        let output: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::HashPath, "hash", input);

        let expected_path = hdk3::hash_path::path::Path::from("foo.bar");

        let expected_hash = tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            holochain_types::entry::EntryHashed::from_content(
                Entry::app((&expected_path).try_into().unwrap()).unwrap(),
            )
            .await
        })
        .into_hash();

        assert_eq!(expected_hash.into_inner(), output.into_inner(),);
    }
}
