use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::CascadeImpl;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

#[tracing::instrument(skip(_ribosome, call_context))]
pub fn must_get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: MustGetAgentActivityInput,
) -> Result<Vec<RegisterAgentActivity>, RuntimeError> {
    tracing::debug!("begin must_get_agent_activity");
    let ret = match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace_deterministic: Permission::Allow,
            ..
        } => {
            let MustGetAgentActivityInput {
                author,
                chain_filter,
            } = input;

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let cascade = match call_context.host_context {
                    HostContext::Validate(_) => {
                        CascadeImpl::from_workspace_stores(workspace.stores(), None)
                    }
                    _ => CascadeImpl::from_workspace_and_network(
                        &workspace,
                        call_context.host_context.network().clone(),
                    ),
                };
                let result = cascade
                    .must_get_agent_activity(author.clone(), chain_filter.clone())
                    .await
                    .map_err(|cascade_error| -> RuntimeError {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string())).into()
                    })?;

                use MustGetAgentActivityResponse::*;

                let result: Result<_, RuntimeError> = match (result, &call_context.host_context) {
                    (Activity(activity), _) => Ok(activity),
                    (IncompleteChain | ChainTopNotFound(_), HostContext::Init(_)) => {
                        Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(InitCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::AgentActivity(author, chain_filter)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                        ))
                        .into())
                    }
                    (IncompleteChain | ChainTopNotFound(_), HostContext::Validate(_)) => {
                        Err(wasm_error!(WasmErrorInner::HostShortCircuit(
                            holochain_serialized_bytes::encode(
                                &ExternIO::encode(ValidateCallbackResult::UnresolvedDependencies(
                                    UnresolvedDependencies::AgentActivity(author, chain_filter)
                                ))
                                .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?,
                            )
                            .map_err(|e| -> RuntimeError { wasm_error!(e).into() })?
                        ))
                        .into())
                    }
                    (IncompleteChain, _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity chain is incomplete for author {} and filter {:?}",
                        author, chain_filter
                    )))
                    .into()),
                    (ChainTopNotFound(missing_action), _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity is missing action {} for author {} and filter {:?}",
                        missing_action, author, chain_filter
                    )))
                    .into()),
                    (EmptyRange, _) => Err(wasm_error!(WasmErrorInner::Host(format!(
                        "must_get_agent_activity chain has produced an invalid range because the range is empty for author {} and filter {:?}",
                        author, chain_filter
                    )))
                    .into()),
                };

                result
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "must_get_agent_activity".into(),
            )
            .to_string(),
        ))
        .into()),
    };
    tracing::debug!(?ret);
    ret
}

#[cfg(test)]
pub mod test {
    use std::sync::Arc;

    use crate::{
        core::ribosome::wasm_test::RibosomeTestFixture,
        sweettest::{SweetConductor, SweetZome},
    };
    use anyhow::Result as Fallible;
    use hdk::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    use self::utils::shared_values::SharedValues;

    mod utils {
        use anyhow::{bail, Result as Fallible};
        use std::{collections::HashMap, sync::Arc};

        pub(crate) mod shared_values {
            use std::{
                f32::consts::E,
                sync::{Condvar, Mutex},
            };

            use serde::{Deserialize, Serialize};

            use super::*;

            static TEST_SHARED_VALUES_TYPE: &str = "TEST_SHARED_VALUES_TYPE";
            static TEST_SHARED_VALUES_TYPE_LOCALV1: &str = "localv1";
            static TEST_SHARED_VALUES_TYPE_REMOTEV1: &str = "remotev1";
            static TEST_SHARED_VALUES_REMOTEV1_URL: &str = "TEST_SHARED_VALUES_REMOTEV1_URL";

            #[derive(Clone, Default)]
            pub(crate) struct LocalV1 {
                notification: Arc<HashMap<String, Arc<Condvar>>>,
                data: HashMap<String, Arc<Mutex<Option<String>>>>,
            }

            #[derive(Clone)]
            pub(crate) struct RemoteV1 {
                url: String,
            }

            /// Represents the message bus used by the agents to exchange information at runtime.
            #[derive(Clone)]
            pub(crate) enum SharedValues {
                LocalV1(LocalV1),
                RemoteV1(RemoteV1),
            }

            impl SharedValues {
                /// Returns a new MessageBus by respecting the environment variables:
                /// TEST_SHARED_VALUES_TYPE: can be either of
                /// - `in`: creates a message bus for in-process messaging
                /// - `remotev1`: creates a message bus for inter-process messaging. relies on another environment variable:
                ///     - TEST_SHARED_VALUES_REMOTEV1_URL: a URL for the remote endpoint to connect the message bus to
                pub(crate) fn new_from_env() -> Fallible<Self> {
                    let bus_type = std::env::var(TEST_SHARED_VALUES_TYPE)
                        .unwrap_or(TEST_SHARED_VALUES_TYPE_LOCALV1.to_string());

                    if &bus_type == TEST_SHARED_VALUES_TYPE_LOCALV1 {
                        Ok(Self::LocalV1(LocalV1::default()))
                    } else if &bus_type == TEST_SHARED_VALUES_TYPE_REMOTEV1 {
                        unimplemented!()
                    } else {
                        bail!("unknown message bus type: {bus_type}")
                    }
                }

                /// Gets the `value` for `key`; waits for it to become available if necessary.
                pub(crate) fn get<T: for<'a> Deserialize<'a>>(&mut self, key: &str) -> Fallible<T> {
                    match self {
                        SharedValues::LocalV1(localv1) => {
                            let mut maybe_value = localv1
                                .data
                                .entry(key.to_string())
                                .or_default()
                                .lock()
                                .expect("mutex poisened");

                            while maybe_value.is_none() {
                                dbg!("waiting for signal on {}", key);
                                maybe_value = localv1
                                    .notification
                                    .entry(key.to_string())
                                    .or_default()
                                    .wait(maybe_value)
                                    .expect("mutex poisoned");
                            }
                            dbg!("got signal on {}", key);

                            if let Some(value) = &*maybe_value {
                                Ok(serde_json::from_str(value)?)
                            } else {
                                unreachable!();
                            }
                        }
                        SharedValues::RemoteV1(_) => unimplemented!(),
                    }
                }

                /// Puts the `value` for `key` and sends it to a receiver if there is one waiting for it.
                pub(crate) fn put<T: Serialize + for<'a> Deserialize<'a>>(
                    &mut self,
                    key: String,
                    value: T,
                ) -> Fallible<Option<T>> {
                    match self {
                        SharedValues::LocalV1(localv1) => {
                            dbg!("putting in value for {}", &key);

                            let mut maybe_value = localv1
                                .data
                                .entry(key.clone())
                                .or_default()
                                .lock()
                                .expect("mutex poisoned");

                            let maybe_previous_value = if let Some(previous_value) = &*maybe_value {
                                serde_json::from_str(previous_value.as_str())?
                            } else {
                                None
                            };

                            *maybe_value = Some(serde_json::to_string(&value)?);

                            if let Some(condvar) = localv1.notification.get(&key) {
                                dbg!("notifying waiters on {}", &key);
                                condvar.notify_all();
                            } else {
                                dbg!("no waiters for {}", &key);
                            }

                            Ok(maybe_previous_value)
                        }
                        SharedValues::RemoteV1(_) => unimplemented!(),
                    }
                }
            }
        }
    }

    /// Mimics inside the must_get wasm.
    #[derive(serde::Serialize, serde::Deserialize, SerializedBytes, Debug, PartialEq)]
    struct Something(#[serde(with = "serde_bytes")] Vec<u8>);

    static A_KEY: &str = "A";
    static B_KEY: &str = "B";
    static C_KEY: &str = "C";
    static D_KEY: &str = "D";
    static BOB_AGENT_PUBKEY_KEY: &str = "bobagentpubkey";
    
    /// Test that validation can get the currently-being-validated agent's
    /// activity.
    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_agent_activity_self() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        // This test is a repro of some issue where the init being inline with
        // the commit being validated may or may not be important. For that
        // reason this test should not be merged with other tests/assertions.
        let _: () = conductor
            .call(&alice, "commit_require_self_agents_chain", ())
            .await;
    }

    /// Test that validation can get the currently-being-validated agent's
    /// previous action bounded activity.
    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_agent_activity_self_prev() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor, alice, ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        // This test is a repro of some issue where the init being inline with
        // the commit being validated may or may not be important. For that
        // reason this test should not be merged with other tests/assertions.
        let _: () = conductor
            .call(&alice, "commit_require_self_prev_agents_chain", ())
            .await;
    }

    async fn bob_fn(
        bob: SweetZome,
        conductor: Arc<SweetConductor>,
        mut shared_values: SharedValues,
    ) -> Fallible<()> {
        shared_values.put(
            BOB_AGENT_PUBKEY_KEY.to_string(),
            bob.cell_id().agent_pubkey().clone(),
        )?;

        shared_values
            .put(
                A_KEY.to_string(),
                conductor
                    .call::<_, ActionHash, _>(&bob, "commit_something", Something(vec![1]))
                    .await,
            )
            .unwrap();

        shared_values
            .put(
                B_KEY.to_string(),
                conductor
                    .call::<_, ActionHash, _>(&bob, "commit_something", Something(vec![2]))
                    .await,
            )
            .unwrap();

        shared_values
            .put(
                C_KEY.to_string(),
                conductor
                    .call::<_, ActionHash, _>(&bob, "commit_something", Something(vec![3]))
                    .await,
            )
            .unwrap();

        for i in 3..30 {
            let _: ActionHash = conductor
                .call(&bob, "commit_something", Something(vec![i]))
                .await;
        }

        let d: ActionHash = conductor
            .call(&bob, "commit_something", Something(vec![21]))
            .await;

        shared_values.put(D_KEY.to_string(), d).unwrap();

        Ok(())
    }

    async fn alice_fn(
        alice: SweetZome,
        conductor: Arc<SweetConductor>,
        mut shared_values: SharedValues,
    ) -> Fallible<()> {
        let bob_agent_pubkey: ActionHash = shared_values.get(&BOB_AGENT_PUBKEY_KEY.to_string())?;

        let a: ActionHash = shared_values.get(&A_KEY.to_string())?;
        let b: ActionHash = shared_values.get(&B_KEY.to_string())?;
        let c: ActionHash = shared_values.get(&C_KEY.to_string())?;

        let filter = ChainFilter::new(a.clone());

        let _: ActionHash = conductor
            .call(
                &alice,
                "commit_require_agents_chain",
                (bob_agent_pubkey.clone(), filter.clone()),
            )
            .await;

        // Try the same filter but on alice's chain.
        // This will fail because alice does not have `a` hash in her chain.
        let err: Result<ActionHash, _> = conductor
            .call_fallible(
                &alice,
                "commit_require_agents_chain",
                (alice.cell_id().agent_pubkey().clone(), filter),
            )
            .await;

        err.unwrap_err();

        let _: ActionHash = conductor
            .call(
                &alice,
                "commit_require_agents_chain_recursive",
                (bob_agent_pubkey.clone(), c.clone()),
            )
            .await;

        let d: ActionHash = shared_values.get(&D_KEY.to_string())?;

        let _: ActionHash = conductor
            .call(
                &alice,
                "commit_require_agents_chain_recursive",
                (bob_agent_pubkey.clone(), d.clone()),
            )
            .await;

        let filter = ChainFilter::new(c.clone()).until(a.clone());

        let r: Vec<RegisterAgentActivity> = conductor
            .call(
                &alice,
                "call_must_get_agent_activity",
                (bob_agent_pubkey.clone(), filter.clone()),
            )
            .await;

        assert_eq!(
            r.into_iter()
                .map(|op| op.action.hashed.hash)
                .collect::<Vec<_>>(),
            vec![c, b, a]
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ribosome_must_get_agent_activity() {
        holochain_trace::test_run().ok();
        let RibosomeTestFixture {
            conductor,
            alice,
            bob,
            ..
        } = RibosomeTestFixture::new(TestWasm::MustGet).await;

        let conductor = Arc::new(conductor);
        let shared_values = utils::shared_values::SharedValues::new_from_env().unwrap();

        let mut handles = vec![];

        // TODO: introduce an env var that specifies a different conductor config
        // TODO: introduce an env var that chooses to selects only one agent to run

        // for (zome, closure) in [(bob, Box::new(bob_fn)), (alice, Box::new(alice_fn))]
        //     as [(_, Box<fn(_, _, _) -> _>); 2]
        // {
        //     let conductor = Arc::clone(&conductor);
        //     let shared_values = shared_values.clone();
        //     handles.push(std::thread::spawn(move || {
        //         holochain_util::tokio_helper::block_forever_on(closure(
        //             zome,
        //             conductor,
        //             shared_values,
        //         ))
        //     }));
        // }

        {
            let conductor = Arc::clone(&conductor);
            let shared_values = shared_values.clone();
            handles.push(std::thread::spawn(move || {
                holochain_util::tokio_helper::block_forever_on(bob_fn(
                    bob,
                    conductor,
                    shared_values,
                ))
            }));
        }

        {
            let conductor = Arc::clone(&conductor);
            let shared_values = shared_values.clone();
            handles.push(std::thread::spawn(move || {
                holochain_util::tokio_helper::block_forever_on(alice_fn(
                    alice,
                    conductor,
                    shared_values,
                ))
            }));
        }

        for handle in handles {
            let _ = handle.join().unwrap();
        }
    }
}
