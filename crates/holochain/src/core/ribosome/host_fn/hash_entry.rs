use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holo_hash::HasHash;
use holochain_types::prelude::*;
use std::sync::Arc;
use holochain_wasmer_host::prelude::WasmError;

pub fn hash_entry(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    input: Entry,
) -> Result<EntryHash, WasmError> {
    let entry_hash = holochain_types::entry::EntryHashed::from_content_sync(input).into_hash();

    Ok(entry_hash)
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {
    use super::*;
    use crate::core::ribosome::host_fn::hash_entry::hash_entry;

    use crate::fixt::CallContextFixturator;
    use crate::fixt::EntryFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use holo_hash::EntryHash;
    use holochain_wasm_test_utils::TestWasm;
    use std::convert::TryInto;
    use std::sync::Arc;

    #[tokio::test(threaded_scheduler)]
    /// we can get an entry hash out of the fn directly
    async fn hash_entry_test() {
        let ribosome = RealRibosomeFixturator::new(crate::fixt::curve::Zomes(vec![]))
            .next()
            .unwrap();
        let call_context = CallContextFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        let input = EntryFixturator::new(::fixt::Predictable).next().unwrap();

        let output: EntryHash =
            hash_entry(Arc::new(ribosome), Arc::new(call_context), input).unwrap();

        assert_eq!(
            *output.hash_type(),
            holo_hash::hash_type::Entry
        );
    }

    #[tokio::test(threaded_scheduler)]
    /// we can get an entry hash out of the fn via. a wasm call
    async fn ribosome_hash_entry_test() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let input = EntryFixturator::new(::fixt::Predictable).next().unwrap();
        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let output: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::HashEntry, "hash_entry", input);
        assert_eq!(
            *output.hash_type(),
            holo_hash::hash_type::Entry
        );

        let entry_hash_output: EntryHash = crate::call_test_ribosome!(host_access, TestWasm::HashEntry, "twenty_three_degrees_entry_hash", ());

        let hash_output: EntryHash = crate::call_test_ribosome!(host_access, TestWasm::HashEntry, "twenty_three_degrees_hash", ());

        assert_eq!(entry_hash_output, hash_output);
    }

    #[tokio::test(threaded_scheduler)]
    /// the hash path underlying anchors wraps entry_hash
    async fn ribosome_hash_path_pwd_test() {
        let test_env = holochain_lmdb::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let input = "foo.bar".to_string();
        let output: EntryHash =
            crate::call_test_ribosome!(host_access, TestWasm::HashPath, "hash", input);

        let expected_path = hdk3::hash_path::path::Path::from("foo.bar");

        let expected_hash = holochain_types::entry::EntryHashed::from_content_sync(
            Entry::app((&expected_path).try_into().unwrap()).unwrap(),
        )
        .into_hash();

        assert_eq!(expected_hash.into_inner(), output.into_inner(),);
    }
}
