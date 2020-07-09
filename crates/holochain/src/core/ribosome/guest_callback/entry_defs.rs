use crate::core::ribosome::FnComponents;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::ZomesToInvoke;
use fixt::prelude::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::dna::zome::HostFnAccess;
use holochain_zome_types::entry_def::EntryDefs;
use holochain_zome_types::entry_def::EntryDefsCallbackResult;
use holochain_zome_types::zome::ZomeName;
use holochain_zome_types::HostInput;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct EntryDefsInvocation;

impl EntryDefsInvocation {
    pub fn new() -> Self {
        Self
    }
}

fixturator!(
    EntryDefsInvocation;
    constructor fn new();
);

impl Invocation for EntryDefsInvocation {
    fn allowed_access(&self) -> HostFnAccess {
        HostFnAccess::none()
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
                // err overrides everything
                (zome_name, EntryDefsCallbackResult::Err(fail_string)) => {
                    Self::Err(zome_name, fail_string)
                }
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
#[cfg(feature = "slow_tests")]
mod test {

    use super::EntryDefsInvocationFixturator;
    use super::EntryDefsResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::RibosomeT;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::curve::Zomes;
    use crate::fixt::EntryDefsFixturator;
    use crate::fixt::WasmRibosomeFixturator;
    use crate::fixt::ZomeNameFixturator;
    use fixt::prelude::*;
    use holochain_serialized_bytes::prelude::*;
    use holochain_types::dna::zome::HostFnAccess;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::crdt::CrdtType;
    use holochain_zome_types::entry_def::EntryDef;
    use holochain_zome_types::entry_def::EntryDefs;
    use holochain_zome_types::entry_def::EntryDefsCallbackResult;
    use holochain_zome_types::entry_def::EntryVisibility;
    use holochain_zome_types::zome::ZomeName;
    use holochain_zome_types::HostInput;
    use matches::assert_matches;
    use std::collections::BTreeMap;

    #[tokio::test(threaded_scheduler)]
    /// this is a non-standard fold test because the result is not so simple
    async fn entry_defs_callback_result_fold() {
        let mut rng = thread_rng();

        let mut zome_name_fixturator = ZomeNameFixturator::new(fixt::Unpredictable);
        let mut entry_defs_fixturator = EntryDefsFixturator::new(fixt::Unpredictable);
        let mut string_fixturator = StringFixturator::new(fixt::Unpredictable);

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

        // some err
        let mut results = vec![];

        let number_of_fails = rng.gen_range(1, 3);
        let number_of_defs = rng.gen_range(0, 3);

        for _ in 0..number_of_fails {
            results.push((
                zome_name_fixturator.next().unwrap(),
                EntryDefsCallbackResult::Err(string_fixturator.next().unwrap()),
            ));
        }

        for _ in 0..number_of_defs {
            results.push((
                zome_name_fixturator.next().unwrap(),
                EntryDefsCallbackResult::Defs(entry_defs_fixturator.next().unwrap()),
            ));
        }

        results.shuffle(&mut rng);

        let result: EntryDefsResult = results.into();

        match result {
            EntryDefsResult::Err(_, _) => assert!(true),
            _ => assert!(false),
        }
    }

    #[tokio::test(threaded_scheduler)]
    async fn entry_defs_invocation_allow_side_effects() {
        use holochain_types::dna::zome::Permission::*;
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Unpredictable)
            .next()
            .unwrap();
        assert_matches!(
            entry_defs_invocation.allowed_access(),
            HostFnAccess {
                side_effects: Deny,
                agent_info: Deny,
                read_workspace: Deny,
                non_determinism: Deny,
            }
        );
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

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_entry_defs_unimplemented() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::Foo]))
            .next()
            .unwrap();
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();

        let result = ribosome.run_entry_defs(entry_defs_invocation).unwrap();
        assert_eq!(result, EntryDefsResult::Defs(BTreeMap::new()),);
    }

    #[tokio::test(threaded_scheduler)]
    #[serial_test::serial]
    async fn test_entry_defs_implemented_defs() {
        let ribosome = WasmRibosomeFixturator::new(Zomes(vec![TestWasm::EntryDefs]))
            .next()
            .unwrap();
        let entry_defs_invocation = EntryDefsInvocationFixturator::new(fixt::Empty)
            .next()
            .unwrap();

        let result = ribosome.run_entry_defs(entry_defs_invocation).unwrap();
        assert_eq!(
            result,
            EntryDefsResult::Defs({
                let mut tree = BTreeMap::new();
                let zome_name: ZomeName = "entry_defs".into();
                let defs: EntryDefs = vec![
                    EntryDef {
                        id: "post".into(),
                        visibility: EntryVisibility::Public,
                        crdt_type: CrdtType,
                        required_validations: 8.into(),
                    },
                    EntryDef {
                        id: "comment".into(),
                        visibility: EntryVisibility::Private,
                        crdt_type: CrdtType,
                        required_validations: 3.into(),
                    },
                ]
                .into();
                tree.insert(zome_name, defs);
                tree
            }),
        );
    }
}
