use super::Workspace;
use super::{error::WorkflowResult, InitializeZomesWorkflow, Workflow, WorkflowEffects};
use crate::core::ribosome::ZomeInvocation;
use crate::core::ribosome::ZomeInvocationResponse;
use crate::core::ribosome::{error::RibosomeResult, RibosomeT};
use crate::core::state::{
    cascade::Cascade,
    chain_cas::ChainCasBuf,
    chain_meta::ChainMetaBuf,
    source_chain::{SourceChain, SourceChainResult},
    workspace::WorkspaceResult,
};
use futures::future::FutureExt;
use holo_hash::{AgentPubKey, HeaderHash};
use holochain_state::prelude::*;
use holochain_types::{cell::CellId, shims::CapToken};
use holochain_zome_types::{zome::ZomeName, HostInput};
use must_future::MustBoxFuture;
use unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;

pub mod unsafe_invoke_zome_workspace;

/// Placeholder for the return value of a zome invocation
/// TODO: do we want this to be the same as ZomeInvocationRESPONSE?
pub type ZomeInvocationResult = RibosomeResult<ZomeInvocationResponse>;

/// Everything needed to call a zome function
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ZomeInvocationExternal {
    /// The ID of the [Cell] in which this Zome-call would be invoked
    pub cell_id: CellId,
    /// The name of the Zome containing the function that would be invoked
    pub zome_name: ZomeName,
    /// The capability request authorization this [ZomeInvocation]
    pub cap: CapToken,
    /// The name of the Zome function to call
    pub fn_name: String,
    /// The serialized data to pass an an argument to the Zome call
    pub payload: HostInput,
    /// the provenance of the call
    pub provenance: AgentPubKey,
}

pub(crate) struct InvokeZomeWorkflow<Ribosome: RibosomeT> {
    pub ribosome: Ribosome,
    pub invocation: ZomeInvocationExternal,
}

impl<'env, Ribosome> Workflow<'env> for InvokeZomeWorkflow<Ribosome>
where
    Ribosome: RibosomeT + Send + Sync + 'env,
{
    type Output = ZomeInvocationResult;
    type Workspace = InvokeZomeWorkspace<'env>;
    type Triggers = Option<InitializeZomesWorkflow>;

    #[allow(unreachable_code)]
    fn workflow(
        self,
        mut workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self>> {
        async {
            let Self {
                ribosome,
                invocation,
            } = self;

            // Check if the initialize workflow has been successfully run
            // TODO: check for existence of initialization-done marker, when implemented
            let triggers = if workspace.source_chain.len() < 4 {
                Some(InitializeZomesWorkflow {})
            } else {
                None
            };

            // Get te current head
            let _chain_head_start = workspace.source_chain.chain_head()?.clone();

            tracing::trace!(line = line!());
            // Create the unsafe sourcechain for use with wasm closure
            let result = {
                let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);

                let as_at = todo!("Maybe this isn't needed?");
                ribosome.call_zome_function(invocation.to_zome_invocation(raw_workspace, as_at))
            };
            tracing::trace!(line = line!());

            // Get the new head
            let _chain_head_end = workspace.source_chain.chain_head()?;

            // Has there been changes?
            // david.b - this isn't doing anything?... commenting out for now
            /*
            if chain_head_start != *chain_head_end {
                // get the changes
                workspace
                    .source_chain
                    .iter_back()
                    .scan(None, |current_header, entry| {
                        let my_header = current_header.clone();
                        *current_header = entry.header().prev_header().cloned();
                        let r = match my_header {
                            Some(current_header) if current_header == chain_head_start => None,
                            _ => Some(entry),
                        };
                        Ok(r)
                    })
                    .map_err(WorkflowError::from)
                    // call the sys validation on the changes etc.
                    .map(|chain_head| {
                        // check_entry_hash(&chain_head.entry_address.into())?
                        Ok(chain_head)
                    })
                    .collect::<Vec<_>>()?;
            }
            */

            let fx = WorkflowEffects {
                workspace,
                callbacks: Default::default(),
                signals: Default::default(),
                triggers,
            };

            Ok((result, fx))
        }
        .boxed()
        .into()
    }
}

pub struct InvokeZomeWorkspace<'env> {
    pub source_chain: SourceChain<'env>,
    pub meta: ChainMetaBuf<'env>,
    pub cache_cas: ChainCasBuf<'env>,
    pub cache_meta: ChainMetaBuf<'env>,
}

impl<'env> InvokeZomeWorkspace<'env> {
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        let source_chain = SourceChain::new(reader, dbs)?;

        let cache_cas = ChainCasBuf::cache(reader, dbs)?;
        let meta = ChainMetaBuf::primary(reader, dbs)?;
        let cache_meta = ChainMetaBuf::cache(reader, dbs)?;

        Ok(InvokeZomeWorkspace {
            source_chain,
            meta,
            cache_cas,
            cache_meta,
        })
    }

    pub fn cascade(&self) -> Cascade {
        Cascade::new(
            &self.source_chain.cas(),
            &self.meta,
            &self.cache_cas,
            &self.cache_meta,
        )
    }
}

impl<'env> Workspace<'env> for InvokeZomeWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.into_inner().flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}

impl ZomeInvocationExternal {
    fn to_zome_invocation(
        self,
        workspace: UnsafeInvokeZomeWorkspace,
        as_at: HeaderHash,
    ) -> ZomeInvocation {
        ZomeInvocation {
            workspace: workspace,
            cell_id: self.cell_id,
            zome_name: self.zome_name,
            cap: self.cap,
            fn_name: self.fn_name,
            payload: self.payload,
            provenance: self.provenance,
            as_at,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::workflow::{effects::WorkflowTriggers, fake_genesis, WorkflowError};
    use holochain_serialized_bytes::prelude::*;
    use holochain_state::{env::ReadManager, test_utils::test_cell_env};
    use holochain_types::{observability, test_utils::fake_agent_pubkey_1};
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::entry::Entry;
    use holochain_zome_types::GuestOutput;
    use holochain_zome_types::HostInput;

    use futures::{future::BoxFuture, FutureExt};
    use matches::assert_matches;

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct Payload {
        a: u32,
    }

    async fn run_invoke_zome<'env, Ribosome: RibosomeT + Send + Sync + 'env>(
        workspace: InvokeZomeWorkspace<'env>,
        ribosome: Ribosome,
        invocation: ZomeInvocationExternal,
    ) -> WorkflowResult<'env, InvokeZomeWorkflow<Ribosome>> {
        let workflow = InvokeZomeWorkflow {
            invocation,
            ribosome,
        };
        workflow.workflow(workspace).await
    }

    // 0.5. Initialization Complete?
    // Check if source chain seq/head ("as at") is less than 4, if so,
    // Call Initialize zomes workflows (which will end up adding an entry
    // for "zome initialization complete") MVI
    #[tokio::test(threaded_scheduler)]
    async fn runs_init() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let mut ribosome = MockRibosomeT::new();

        // Genesis
        fake_genesis(&mut workspace.source_chain).await;

        // Setup the ribosome mock
        ribosome
            .expect_call_zome_function()
            .returning(move |_invocation| {
                let x = SerializedBytes::try_from(Payload { a: 3 }).unwrap();
                Ok(ZomeInvocationResponse::ZomeApiFn(GuestOutput::new(x)))
            });

        let invocation = crate::core::ribosome::ZomeInvocationFixturator::new(
            crate::core::ribosome::NamedInvocation(
                holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                HostInput::new(Payload { a: 1 }.try_into().unwrap()),
            ),
        )
        .next()
        .unwrap();

        let workflow = InvokeZomeWorkflow {
            invocation,
            ribosome,
        };
        let (_, effects) = workflow.workflow(workspace).await.unwrap();

        // Check the initialize zome was added to a trigger
        assert!(effects.signals.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(!effects.triggers.is_empty());
        assert_matches!(effects.triggers, Some(InitializeZomesWorkflow {}));
    }

    // 1.  Check if there is a Capability token secret in the parameters.
    // If there isn't and the function to be called isn't public,
    // we stop the process and return an error. MVT
    // TODO: B-01553: Finish this test when capabilities land
    #[ignore]
    #[allow(unused_variables, unreachable_code)]
    #[tokio::test]
    async fn private_zome_call() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let ribosome = MockRibosomeT::new();
        // FIXME: CAP: Set this function to private
        let invocation = crate::core::ribosome::ZomeInvocationFixturator::new(
            crate::core::ribosome::NamedInvocation(
                holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                HostInput::new(Payload { a: 1 }.try_into().unwrap()),
            ),
        )
        .next()
        .unwrap();
        invocation.cap = todo!("Make secret cap token");
        let error = run_invoke_zome(workspace, ribosome, invocation)
            .await
            .unwrap_err();
        assert_matches!(error, WorkflowError::CapabilityMissing);
    }

    // TODO: B-01553: Finish these tests when capabilities land
    // 1.1 If there is a secret, we look up our private CAS and see if it matches any secret for a
    // Capability Grant entry that we have stored. If it does, check that this Capability Grant is
    //not revoked and actually grants permissions to call the ZomeFn that is being called. (MVI)

    // 1.2 Check if the Capability Grant has assignees=None (means this Capability is transferable).
    // If it has assignees=Vec<Address> (means this Capability is on Assigned mode, check that the
    // provenance's agent key is in that assignees. (MVI)

    // 1.3 If the CapabiltyGrant has pre-filled parameters, check that the ui is passing exactly the
    // parameters needed and no more to complete the call. (MVI)

    // 2. Set Context (Cascading Cursor w/ Pre-flight chain extension) MVT

    // 3. Invoke WASM (w/ Cursor) MVM
    // WASM receives external call handles:
    // (gets & commits via cascading cursor, crypto functions & bridge calls via conductor,
    // send via network function call for send direct message)

    // There is no test for `3.` only that it compiles

    // 4. When the WASM code execution finishes, If workspace has new chain entries:
    // 4.1. Call system validation of list of entries and headers: (MVI)
    // - Check entry hash
    // - Check header hash
    // - Check header signature
    // - Check header timestamp is later than previous timestamp
    // - Check entry content matches entry schema
    //   Depending on the type of the commit, validate all possible validations for the
    //   DHT Op that would be produced by it
    // TODO: B-01092: SYSTEM_VALIDATION: Finish when sys val lands
    #[ignore]
    #[tokio::test]
    async fn calls_system_validation<'a>() {
        observability::test_run().ok();
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

        // Genesis
        let agent_header = fake_genesis(&mut workspace.source_chain).await;

        let agent_pubkey = fake_agent_pubkey_1();
        let agent_entry = Entry::Agent(agent_pubkey.clone().into());
        let mut ribosome = MockRibosomeT::new();
        // Call zome mock that it writes to source chain
        ribosome
            .expect_call_zome_function()
            .returning(move |_invocation| {
                let agent_header = agent_header.clone();
                let agent_entry = agent_entry.clone();
                let _call = |workspace: &'a mut InvokeZomeWorkspace| -> BoxFuture<'a, ()> {
                    async move {
                        workspace
                            .source_chain
                            .put(agent_header.clone(), Some(agent_entry))
                            .await
                            .unwrap();
                    }
                    .boxed()
                };
                /* FIXME: Mockall doesn't seem to work with async?
                unsafe { unsafe_workspace.apply_mut(call).await };
                */
                let x = SerializedBytes::try_from(Payload { a: 3 }).unwrap();
                Ok(ZomeInvocationResponse::ZomeApiFn(GuestOutput::new(x)))
            });

        let invocation = crate::core::ribosome::ZomeInvocationFixturator::new(
            crate::core::ribosome::NamedInvocation(
                holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                HostInput::new(Payload { a: 1 }.try_into().unwrap()),
            ),
        )
        .next()
        .unwrap();
        // IDEA: Mock the system validation and check it's called
        /* This is one way to test the correctness of the calls to sys val
        let mut sys_val = MockSystemValidation::new();
        sys_val
            .expect_check_entry_hash()
            .times(1)
            .returning(|_entry_hash| Ok(()));
        */

        let (_result, effects) = run_invoke_zome(workspace, ribosome, invocation)
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(effects.signals.is_empty());
    }

    // 4.2. Call app validation of list of entries and headers: (MVI)
    // - Call validate_set_of_entries_and_headers (any necessary get
    //   results where we receive None / Timeout on retrieving validation dependencies, should produce error/fail)
    // TODO: B-01093: Finish when app val lands
    #[ignore]
    #[tokio::test]
    async fn calls_app_validation() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let ribosome = MockRibosomeT::new();
        let invocation = crate::core::ribosome::ZomeInvocationFixturator::new(
            crate::core::ribosome::NamedInvocation(
                holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                HostInput::new(Payload { a: 1 }.try_into().unwrap()),
            ),
        )
        .next()
        .unwrap();
        // TODO: B-01093: Mock the app validation and check it's called
        // TODO: B-01093: How can I pass a app validation into this?
        // These are just static calls
        let (_result, effects) = run_invoke_zome(workspace, ribosome, invocation)
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(effects.signals.is_empty());
    }

    // 4.3. Write output results via SC gatekeeper (wrap in transaction): (MVI)
    // This is handled by the workflow runner however I should test that
    // we can create outputs
    #[ignore]
    #[tokio::test]
    async fn creates_outputs() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let ribosome = MockRibosomeT::new();
        // TODO: Make this mock return an output
        let invocation = crate::core::ribosome::ZomeInvocationFixturator::new(
            crate::core::ribosome::NamedInvocation(
                holochain_types::cell::CellIdFixturator::new(fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                HostInput::new(Payload { a: 1 }.try_into().unwrap()),
            ),
        )
        .next()
        .unwrap();
        let (_result, effects) = run_invoke_zome(workspace, ribosome, invocation)
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(effects.signals.is_empty());
        // TODO: Check the workspace has changes
    }
}
