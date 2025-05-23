use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct EntryDefsInvocation;

impl EntryDefsInvocation {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }
}

#[derive(Clone, Constructor, Debug)]
pub struct EntryDefsHostAccess;

impl From<&HostContext> for EntryDefsHostAccess {
    fn from(_: &HostContext) -> Self {
        Self
    }
}

impl From<EntryDefsHostAccess> for HostContext {
    fn from(entry_defs_host_access: EntryDefsHostAccess) -> Self {
        Self::EntryDefs(entry_defs_host_access)
    }
}

impl From<&EntryDefsHostAccess> for HostFnAccess {
    fn from(_: &EntryDefsHostAccess) -> Self {
        Self::none()
    }
}

impl Invocation for EntryDefsInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::AllIntegrity
    }
    fn fn_components(&self) -> FnComponents {
        vec!["entry_defs".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(())
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

/// the aggregate result of _all_ entry defs callbacks
#[derive(PartialEq, Debug, Clone)]
pub enum EntryDefsResult {
    /// simple mapping between zome and defs
    Defs(BTreeMap<ZomeName, EntryDefs>),
    Err(ZomeName, String),
}

impl From<Vec<(ZomeName, EntryDefsCallbackResult)>> for EntryDefsResult {
    fn from(callback_results: Vec<(ZomeName, EntryDefsCallbackResult)>) -> Self {
        callback_results.into_iter().fold(
            EntryDefsResult::Defs(BTreeMap::new()),
            |acc, x| match x {
                // passing callback allows the acc to carry forward
                (zome_name, EntryDefsCallbackResult::Defs(defs)) => match acc {
                    Self::Defs(mut btreemap) => {
                        btreemap.insert(zome_name, defs);
                        Self::Defs(btreemap)
                    }
                    Self::Err(_, _) => acc,
                },
            },
        )
    }
}

#[cfg(test)]
mod test {
    use super::EntryDefsHostAccess;
    use super::EntryDefsResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::EntryDefsFixturator;
    use crate::fixt::EntryDefsInvocationFixturator;
    use crate::fixt::ZomeNameFixturator;
    use holochain_types::prelude::*;
    use std::collections::BTreeMap;

    #[test]
    /// this is a non-standard fold test because the result is not so simple
    fn entry_defs_callback_result_fold() {
        let mut zome_name_fixturator = ZomeNameFixturator::new(::fixt::Unpredictable);
        let mut entry_defs_fixturator = EntryDefsFixturator::new(::fixt::Unpredictable);

        // zero defs
        assert_eq!(EntryDefsResult::Defs(BTreeMap::new()), vec![].into(),);

        // one defs
        let zome_name = zome_name_fixturator.next().unwrap();
        let entry_defs = entry_defs_fixturator.next().unwrap();
        assert_eq!(
            EntryDefsResult::Defs({
                let mut tree = BTreeMap::new();
                tree.insert(zome_name.clone(), entry_defs.clone());
                tree
            }),
            vec![(zome_name, EntryDefsCallbackResult::Defs(entry_defs)),].into(),
        );

        // two defs
        let zome_name_one = zome_name_fixturator.next().unwrap();
        let entry_defs_one = entry_defs_fixturator.next().unwrap();
        let zome_name_two = zome_name_fixturator.next().unwrap();
        let entry_defs_two = entry_defs_fixturator.next().unwrap();
        assert_eq!(
            EntryDefsResult::Defs({
                let mut tree = BTreeMap::new();
                tree.insert(zome_name_one.clone(), entry_defs_one.clone());
                tree.insert(zome_name_two.clone(), entry_defs_two.clone());
                tree
            }),
            vec![
                (zome_name_one, EntryDefsCallbackResult::Defs(entry_defs_one)),
                (zome_name_two, EntryDefsCallbackResult::Defs(entry_defs_two)),
            ]
            .into()
        );
    }

    #[test]
    fn entry_defs_host_access() {
        assert_eq!(
            HostFnAccess::from(&EntryDefsHostAccess),
            HostFnAccess::none()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn entry_defs_invocation_zomes() {
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(ZomesToInvoke::AllIntegrity, entry_defs_invocation.zomes(),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn entry_defs_invocation_fn_components() {
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["entry_defs"];
        for fn_component in entry_defs_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn entry_defs_invocation_host_input() {
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let host_input = entry_defs_invocation.clone().host_input().unwrap();

        assert_eq!(host_input, ExternIO::encode(()).unwrap());
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use crate::core::ribosome::guest_callback::entry_defs::EntryDefsHostAccess;
    use crate::core::ribosome::guest_callback::entry_defs::EntryDefsResult;
    use crate::core::ribosome::wasm_test::RibosomeTestFixture;
    use crate::core::ribosome::RibosomeT;
    use crate::fixt::EntryDefsInvocationFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::Zomes;
    use holochain_types::prelude::*;
    use holochain_wasm_test_utils::TestWasm;
    pub use holochain_zome_types::entry_def::EntryVisibility;
    use std::collections::BTreeMap;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_entry_defs_unimplemented() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();

        let result = ribosome
            .run_entry_defs(EntryDefsHostAccess, entry_defs_invocation)
            .await
            .unwrap();
        assert_eq!(result, EntryDefsResult::Defs(BTreeMap::new()),);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_entry_defs_index_lookup() {
        holochain_trace::test_run();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::EntryDefs).await;

        let _: () = conductor.call(&alice, "assert_indexes", ()).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_entry_defs_implemented_defs() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::EntryDefs]))
            .next()
            .unwrap();
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(::fixt::Empty)
            .next()
            .unwrap();

        let result = ribosome
            .run_entry_defs(EntryDefsHostAccess, entry_defs_invocation)
            .await
            .unwrap();
        assert_eq!(
            result,
            EntryDefsResult::Defs({
                let mut tree = BTreeMap::new();
                let zome_name: ZomeName = "integrity_entry_defs".into();
                let defs: EntryDefs = vec![
                    EntryDef {
                        id: "post".into(),
                        visibility: EntryVisibility::Public,
                        required_validations: 5.into(),
                        ..Default::default()
                    },
                    EntryDef {
                        id: "comment".into(),
                        visibility: EntryVisibility::Private,
                        required_validations: 5.into(),
                        ..Default::default()
                    },
                ]
                .into();
                tree.insert(zome_name, defs);
                tree
            }),
        );
    }
}
