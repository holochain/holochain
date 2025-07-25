use crate::conductor::api::CellConductorReadHandle;
use crate::core::ribosome::FnComponents;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::Invocation;
use crate::core::ribosome::InvocationAuth;
use crate::core::ribosome::ZomesToInvoke;
use derive_more::Constructor;
use holochain_keystore::MetaLairClient;
use holochain_p2p::DynHolochainP2pDna;
use holochain_serialized_bytes::prelude::*;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_types::prelude::*;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct InitInvocation {
    pub dna_def: DnaDef,
}

impl InitInvocation {
    pub fn new(dna_def: DnaDef) -> Self {
        Self { dna_def }
    }
}

#[derive(Clone, Constructor)]
pub struct InitHostAccess {
    pub workspace: HostFnWorkspace,
    pub keystore: MetaLairClient,
    pub network: DynHolochainP2pDna,
    pub signal_tx: broadcast::Sender<Signal>,
    pub call_zome_handle: CellConductorReadHandle,
}

impl std::fmt::Debug for InitHostAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InitHostAccess").finish()
    }
}

impl From<InitHostAccess> for HostContext {
    fn from(init_host_access: InitHostAccess) -> Self {
        Self::Init(init_host_access)
    }
}

impl From<&InitHostAccess> for HostFnAccess {
    fn from(_: &InitHostAccess) -> Self {
        Self::all()
    }
}

impl Invocation for InitInvocation {
    fn zomes(&self) -> ZomesToInvoke {
        ZomesToInvoke::All
    }
    fn fn_components(&self) -> FnComponents {
        vec!["init".into()].into()
    }
    fn host_input(self) -> Result<ExternIO, SerializedBytesError> {
        ExternIO::encode(())
    }
    fn auth(&self) -> InvocationAuth {
        InvocationAuth::LocalCallback
    }
}

impl TryFrom<InitInvocation> for ExternIO {
    type Error = SerializedBytesError;
    fn try_from(_: InitInvocation) -> Result<Self, Self::Error> {
        Self::encode(())
    }
}

/// the aggregate result of _all_ init callbacks
#[derive(PartialEq, Debug)]
pub enum InitResult {
    /// all init callbacks passed
    Pass,
    /// some init failed
    /// ZomeName is the first zome that failed to init
    /// String is a human-readable error string giving the reason for failure
    Fail(ZomeName, String),
    /// no init failed but some zome has unresolved dependencies
    /// ZomeName is the first zome that has unresolved dependencies
    /// `Vec<EntryHash>` is the list of all missing dependency addresses
    UnresolvedDependencies(ZomeName, UnresolvedDependencies),
}

impl From<Vec<(ZomeName, InitCallbackResult)>> for InitResult {
    fn from(callback_results: Vec<(ZomeName, InitCallbackResult)>) -> Self {
        callback_results
            .into_iter()
            .fold(Self::Pass, |acc, (zome_name, x)| match x {
                // fail overrides everything
                InitCallbackResult::Fail(fail_string) => Self::Fail(zome_name, fail_string),
                // unresolved deps overrides pass but not fail
                InitCallbackResult::UnresolvedDependencies(ud) => match acc {
                    Self::Fail(_, _) => acc,
                    _ => Self::UnresolvedDependencies(zome_name, ud),
                },
                // passing callback allows the acc to carry forward
                InitCallbackResult::Pass => acc,
            })
    }
}

#[cfg(test)]
mod test {
    use super::InitResult;
    use crate::core::ribosome::Invocation;
    use crate::core::ribosome::ZomesToInvoke;
    use crate::fixt::InitHostAccessFixturator;
    use crate::fixt::InitInvocationFixturator;
    use crate::fixt::ZomeNameFixturator;
    use ::fixt::prelude::*;
    use holochain_types::prelude::*;

    #[test]
    fn init_callback_result_fold() {
        let mut rng = ::fixt::rng();

        let result_pass = || InitResult::Pass;
        let result_ud = || {
            InitResult::UnresolvedDependencies(
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                UnresolvedDependencies::Hashes(vec![]),
            )
        };
        let result_fail = || {
            InitResult::Fail(
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                "".into(),
            )
        };

        let cb_pass = || {
            (
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                InitCallbackResult::Pass,
            )
        };
        let cb_ud = || {
            (
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                InitCallbackResult::UnresolvedDependencies(UnresolvedDependencies::Hashes(vec![])),
            )
        };
        let cb_fail = || {
            (
                ZomeNameFixturator::new(::fixt::Predictable).next().unwrap(),
                InitCallbackResult::Fail("".into()),
            )
        };

        for (mut results, expected) in vec![
            (vec![], result_pass()),
            (vec![cb_pass()], result_pass()),
            (vec![cb_fail()], result_fail()),
            (vec![cb_ud()], result_ud()),
            (vec![cb_fail(), cb_pass()], result_fail()),
            (vec![cb_fail(), cb_ud()], result_fail()),
            (vec![cb_pass(), cb_ud()], result_ud()),
            (vec![cb_pass(), cb_fail(), cb_ud()], result_fail()),
        ] {
            // order of the results should not change the final result
            results.shuffle(&mut rng);

            // number of times a callback result appears should not change the final result
            let number_of_extras = rng.random_range(0..5);
            for _ in 0..number_of_extras {
                let maybe_extra = results.choose(&mut rng).cloned();
                if let Some(extra) = maybe_extra {
                    results.push(extra);
                }
            }

            assert_eq!(expected, results.into(),);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn init_access() {
        let init_host_access = InitHostAccessFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(HostFnAccess::from(&init_host_access), HostFnAccess::all(),);
    }

    #[test]
    fn init_invocation_zomes() {
        let init_invocation = InitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();
        assert_eq!(ZomesToInvoke::All, init_invocation.zomes(),);
    }

    #[test]
    fn init_invocation_fn_components() {
        let init_invocation = InitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let mut expected = vec!["init"];
        for fn_component in init_invocation.fn_components() {
            assert_eq!(fn_component, expected.pop().unwrap());
        }
    }

    #[test]
    fn init_invocation_host_input() {
        let init_invocation = InitInvocationFixturator::new(::fixt::Unpredictable)
            .next()
            .unwrap();

        let host_input = init_invocation.clone().host_input().unwrap();

        assert_eq!(host_input, ExternIO::encode(()).unwrap(),);
    }
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
mod slow_tests {
    use super::InitResult;
    use crate::conductor::api::error::ConductorApiError;
    use crate::conductor::api::error::ConductorApiResult;
    use crate::conductor::CellError;
    use crate::core::ribosome::guest_callback::ValidateCallbackResult;
    use crate::core::ribosome::RibosomeError;
    use crate::core::ribosome::RibosomeT;
    use crate::core::workflow::WorkflowError;
    use crate::fixt::InitHostAccessFixturator;
    use crate::fixt::InitInvocationFixturator;
    use crate::fixt::RealRibosomeFixturator;
    use crate::fixt::Zomes;
    use crate::sweettest::SweetConductor;
    use crate::sweettest::SweetDnaFile;
    use crate::sweettest::SweetZome;
    use crate::test_utils::host_fn_caller::Post;
    use ::fixt::prelude::*;
    use assert2::{assert, let_assert};
    use holo_hash::ActionHash;
    use holochain_types::app::DisableCloneCellPayload;
    use holochain_types::inline_zome::InlineZomeSet;
    use holochain_types::prelude::CreateCloneCellPayload;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::clone::CloneCellId;
    use holochain_zome_types::prelude::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_unimplemented() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::Crud]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome
            .run_init(host_access, init_invocation)
            .await
            .unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_implemented_pass() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome
            .run_init(host_access, init_invocation)
            .await
            .unwrap();
        assert_eq!(result, InitResult::Pass,);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_implemented_fail() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::InitFail]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome
            .run_init(host_access, init_invocation)
            .await
            .unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_multi_implemented_fail() {
        let ribosome =
            RealRibosomeFixturator::new(Zomes(vec![TestWasm::InitPass, TestWasm::InitFail]))
                .next()
                .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let result = ribosome
            .run_init(host_access, init_invocation)
            .await
            .unwrap();
        assert_eq!(
            result,
            InitResult::Fail(TestWasm::InitFail.into(), "because i said so".into()),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_implemented_invalid_return() {
        let unit_entry_def = EntryDef::default_from_id("unit");
        let zome = InlineZomeSet::new_unique_single(
            "integrity",
            "coordinator",
            vec![unit_entry_def.clone()],
            0,
        )
        .function("integrity", "validate", |_api, _op: Op| {
            Ok(ValidateCallbackResult::Valid)
        })
        .function("coordinator", "init", |_api, ()| Ok(()))
        .function("coordinator", "create", move |api, ()| {
            let entry = Entry::app(().try_into().unwrap()).unwrap();
            let hash = api.create(CreateInput::new(
                InlineZomeSet::get_entry_location(&api, EntryDefIndex(0)),
                EntryVisibility::Public,
                entry,
                ChainTopOrdering::default(),
            ))?;
            Ok(hash)
        });

        let dnas = [SweetDnaFile::unique_from_inline_zomes(zome).await.0];
        let mut conductor = SweetConductor::from_standard_config().await;
        let app = conductor.setup_app("app", &dnas).await.unwrap();
        let conductor = Arc::new(conductor);
        let (cell,) = app.into_tuple();

        let err = conductor
            .call_fallible::<_, ()>(&cell.zome("coordinator"), "create", ())
            .await
            .unwrap_err();

        let_assert!(ConductorApiError::CellError(CellError::WorkflowError(workflow_err)) = err);

        let_assert!(
            WorkflowError::RibosomeError(RibosomeError::CallbackInvalidReturnType(err_msg)) =
                *workflow_err
        );

        assert_eq!(
            err_msg,
            "invalid type: unit value, expected variant identifier"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_init_implemented_invalid_params() {
        let ribosome = RealRibosomeFixturator::new(Zomes(vec![TestWasm::InitInvalidParams]))
            .next()
            .unwrap();
        let mut init_invocation = InitInvocationFixturator::new(::fixt::Empty).next().unwrap();
        init_invocation.dna_def = ribosome.dna_file.dna_def().clone();

        let host_access = fixt!(InitHostAccess);
        let err = ribosome
            .run_init(host_access, init_invocation)
            .await
            .unwrap_err();

        let_assert!(RibosomeError::CallbackInvalidParameters(err_msg) = err);
        assert!(err_msg == String::default());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conductor_will_not_accept_zome_calls_before_the_network_is_initialised() {
        let (dna_file, _, _) = SweetDnaFile::from_test_wasms(
            random_network_seed(),
            vec![TestWasm::Create],
            SerializedBytes::default(),
        )
        .await;

        let mut conductor = SweetConductor::from_standard_config().await;

        let app = conductor.setup_app("app", [&dna_file]).await.unwrap();

        let cloned = conductor
            .create_clone_cell(
                app.installed_app_id(),
                CreateCloneCellPayload {
                    role_name: dna_file.dna_hash().to_string().clone(),
                    modifiers: DnaModifiersOpt::none()
                        .with_network_seed("anything else".to_string()),
                    membrane_proof: None,
                    name: Some("cloned".to_string()),
                },
            )
            .await
            .unwrap();

        let enable_or_disable_payload = DisableCloneCellPayload {
            clone_cell_id: CloneCellId::CloneId(cloned.clone_id.clone()),
        };
        conductor
            .disable_clone_cell(app.installed_app_id(), &enable_or_disable_payload)
            .await
            .unwrap();

        let zome: SweetZome = SweetZome::new(
            cloned.cell_id.clone(),
            TestWasm::Create.coordinator_zome_name(),
        );

        // Run the cell enable in parallel. If we wait for it then we shouldn't see the error we're looking for
        let conductor_handle = conductor.raw_handle().clone();
        let payload = enable_or_disable_payload.clone();
        tokio::spawn(async move {
            conductor_handle
                .enable_clone_cell(app.installed_app_id(), &payload)
                .await
                .unwrap();
        });

        let mut had_successful_zome_call = false;
        for _ in 0..30 {
            let create_post_result: ConductorApiResult<ActionHash> = conductor
                .call_fallible(&zome, "create_post", Post("clone message".to_string()))
                .await;

            match create_post_result {
                Err(crate::conductor::api::error::ConductorApiError::ConductorError(
                    crate::conductor::error::ConductorError::CellDisabled(_),
                )) => {
                    // Expected errors
                }
                Ok(_) => {
                    had_successful_zome_call = true;

                    // Stop trying after the first successful zome call
                    break;
                }
                Err(e) => {
                    panic!("Other types of error are not expected {:?}", e);
                }
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        assert!(
            had_successful_zome_call,
            "Should have seen a clone cell join the network and allow calls"
        );
    }
}
