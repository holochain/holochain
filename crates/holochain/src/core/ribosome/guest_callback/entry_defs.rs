use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use fixt::prelude::*;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use holochain_zome_types::entry_def::EntryDefs;
use std::collections::BTreeMap;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;

#[derive(Debug, Clone)]
pub struct EntryDefsInvocation {
}

impl EntryDefsInvocation {
    pub fn new() -> Self {
        Self { }
    }
}

fixturator!(
    EntryDefsInvocation;
    constructor fn new();
);

impl Invocation for EntryDefsInvocation {
    fn allow_side_effects(&self) -> bool {
        false
    }
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::All
    }
    fn fn_components(&self) -> FnComponents {
        vec!["entry_defs".into()].into()
    }
    fn host_input(self) -> Result<HostInput, SerializedBytesError> {
        Ok(HostInput::new(().try_into()?))
    }
}

impl TryFrom<EntryDefsInvocation> for HostInput {
    type Error = SerializedBytesError;
    fn try_from(_: EntryDefsInvocation) -> Result<Self, Self::Error> {
        Ok(Self::new(().try_into()?))
    }
}

/// the aggregate result of _all_ entry defs callbacks
#[derive(PartialEq, Debug)]
pub enum EntryDefsResult {
    /// simple mapping between zome and defs
    Defs(BTreeMap<ZomeName, EntryDefs>),
    Err(ZomeName, String),
}

impl From<Vec<EntryDefsCallbackResult>> for EntryDefsResult {
    fn from(callback_results: Vec<EntryDefsCallbackResult>) -> Self {
        callback_results
            .into_iter()
            .fold(EntryDefsResult::Defs(BTreeMap::new()), |acc, x| match x {
                // err overrides everything
                EntryDefsCallbackResult::Err(zome_name, fail_string) => {
                    Self::Err(zome_name, fail_string)
                },
                // passing callback allows the acc to carry forward
                EntryDefsCallbackResult::Defs(zome_name, defs) => {
                    match acc {
                        Self::Defs(mut btreemap) => {
                            btreemap.insert(zome_name, defs);
                            Self::Defs(btreemap)
                        },
                        Self::Err(_, _) => acc,
                    }
                },
            })
    }
}

#[cfg(test)]
mod test {

    use super::EntryDefsInvocationFixturator;
    use super::EntryDefsResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    // use crate::core::workflow::unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspaceFixturator;
    // use crate::fixt::curve::Zomes;
    // use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::EntryDefsFixturator;
    use crate::fixt::ZomeNameFixturator;
    use holochain_serialized_bytes::prelude::*;
    // use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::entry_def::EntryDefsCallbackResult;
    use holochain_zome_types::HostInput;
    use std::collections::BTreeMap;
    // use crate::core::ribosome::RibosomeT;

    #[tokio::test(threaded_scheduler)]
    /// this is a non-standard fold test because the result is not so simple
    async fn entry_defs_callback_result_fold() {
        let mut zome_name_fixturator = ZomeNameFixturator::new(fixt::Unpredictable);
        let mut entry_defs_fixturator = EntryDefsFixturator::new(fixt::Unpredictable);

        // zero defs
        assert_eq!(
            EntryDefsResult::Defs(BTreeMap::new()),
            vec![].into(),
        );

        // one defs
        let zome_name = zome_name_fixturator.next().unwrap();
        let entry_defs = entry_defs_fixturator.next().unwrap();
        assert_eq!(
            EntryDefsResult::Defs({
                let mut tree = BTreeMap::new();
                tree.insert(
                    zome_name.clone(),
                    entry_defs.clone(),
                );
                tree
            }),
            vec![
                EntryDefsCallbackResult::Defs(zome_name, entry_defs),
            ].into(),
        );


        // let result_defs = || EntryDefsResult::Defs({
        //     let mut tree = BTreeMap::new();
        //     tree.insert(
        //         zome_name_fixturator.next().unwrap(),
        //         entry_defs_fixturator.next().unwrap(),
        //     );
        //     tree.insert(
        //         zome_name_fixturator.next().unwrap(),
        //         entry_defs_fixturator.next().unwrap(),
        //     );
        //     tree
        // });
        // let result_error = || {
        //     EntryDefsResult::Err(
        //         ZomeNameFixturator::new(fixt::Predictable).next().unwrap(),
        //         "".into(),
        //     )
        // };
        //
        // let cb_defs = || EntryDefsCallbackResult::Defs(
        //     ZomeNameFixturator::new(fixt::Predictable).next().unwrap(),
        //     EntryDefsFixturator::new(fixt::Predictable).next().unwrap(),
        // );
        // let cb_fail = || {
        //     EntryDefsCallbackResult::Err(
        //         ZomeNameFixturator::new(fixt::Predictable).next().unwrap(),
        //         "".into(),
        //     )
        // };
        //
        // for (mut results, expected) in vec![
        //     (vec![], result_defs()),
        //     (vec![cb_defs()], result_defs()),
        //     (vec![cb_fail()], result_error()),
        //     (vec![cb_fail(), cb_defs()], result_error()),
        // ] {
        //     // order of the results should not change the final result
        //     results.shuffle(&mut rng);
        //
        //     // number of times a callback result appears should not change the final result
        //     let number_of_extras = rng.gen_range(0, 5);
        //     for _ in 0..number_of_extras {
        //         let maybe_extra = results.choose(&mut rng).cloned();
        //         match maybe_extra {
        //             Some(extra) => results.push(extra),
        //             _ => {}
        //         };
        //     }
        //
        //     assert_eq!(expected, results.into(),);
        // }
    }

    #[tokio::test(threaded_scheduler)]
    async fn entry_defs_invocation_allow_side_effects() {
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        assert!(!entry_defs_invocation.allow_side_effects());
    }

    #[tokio::test(threaded_scheduler)]
    async fn entry_defs_invocation_zomes() {
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(ZomesToInvoke::All, entry_defs_invocation.zomes(),);
    }

    #[tokio::test(threaded_scheduler)]
    async fn entry_defs_invocation_fn_components() {
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["entry_defs"];
        for fn_component in entry_defs_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn entry_defs_invocation_host_input() {
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();

        let host_input = entry_defs_invocation.clone().host_input().unwrap();

        assert_eq!(
            host_input,
            HostInput::new(SerializedBytes::try_from(()).unwrap()),
        );
    }

    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // async fn test_entry_defs_unimplemented() {
    //     let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
    //         .next()
    //         .unwrap();
    //     let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
    //         .next()
    //         .unwrap();
    //     let mut entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Empty).next().unwrap();
    //
    //     let result = ribosome.run_entry_defs(workspace, entry_defs_invocation).unwrap();
    //     assert_eq!(result, EntryDefsResult::Pass,);
    // }
    //
    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // async fn test_entry_defs_implemented_pass() {
    //     let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
    //         .next()
    //         .unwrap();
    //     let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::EntryDefsPass]))
    //         .next()
    //         .unwrap();
    //     let mut entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Empty).next().unwrap();
    //
    //     let result = ribosome.run_entry_defs(workspace, entry_defs_invocation).unwrap();
    //     assert_eq!(result, EntryDefsResult::Pass,);
    // }
    //
    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // async fn test_entry_defs_implemented_fail() {
    //     let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
    //         .next()
    //         .unwrap();
    //     let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::EntryDefsFail]))
    //         .next()
    //         .unwrap();
    //     let mut entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Empty).next().unwrap();
    //
    //     let result = ribosome.run_entry_defs(workspace, entry_defs_invocation).unwrap();
    //     assert_eq!(
    //         result,
    //         EntryDefsResult::Fail(TestWasm::EntryDefsFail.into(), "because i said so".into()),
    //     );
    // }
    //
    // #[tokio::test(threaded_scheduler)]
    // #[serial_test::serial]
    // async fn test_entry_defs_multi_implemented_fail() {
    //     let workspace = UnsafeInvokeZomeWorkspaceFixturator::new(fixt::Unpredictable)
    //         .next()
    //         .unwrap();
    //     let ribosome =
    //         WasmRibosomeFixturator::new(Zomes(vec![TestWasm::EntryDefsPass, TestWasm::EntryDefsFail]))
    //             .next()
    //             .unwrap();
    //     let mut entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Empty).next().unwrap();
    //
    //     let result = ribosome.run_entry_defs(workspace, entry_defs_invocation).unwrap();
    //     assert_eq!(
    //         result,
    //         EntryDefsResult::Fail(TestWasm::EntryDefsFail.into(), "because i said so".into()),
    //     );
    // }
}
