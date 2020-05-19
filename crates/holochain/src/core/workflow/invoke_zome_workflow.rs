use super::Workspace;
use super::{
    error::{WorkflowError, WorkflowResult},
    InitializeZomesWorkflow, Workflow, WorkflowEffects,
};
use crate::core::ribosome::{error::RibosomeResult, RibosomeT};
use crate::core::{
    state::{
        cascade::Cascade, chain_cas::ChainCasBuf, chain_meta::ChainMetaBuf,
        source_chain::SourceChain, workspace::WorkspaceResult,
    },
    sys_validate_element,
};
use fallible_iterator::FallibleIterator;
use futures::future::FutureExt;
use holochain_state::prelude::*;
use holochain_types::nucleus::{ZomeInvocation, ZomeInvocationResponse};
use must_future::MustBoxFuture;
use unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace;

pub mod unsafe_invoke_zome_workspace;

/// Placeholder for the return value of a zome invocation
/// TODO: do we want this to be the same as ZomeInvocationRESPONSE?
pub type ZomeInvocationResult = RibosomeResult<ZomeInvocationResponse>;

pub(crate) struct InvokeZomeWorkflow<Ribosome: RibosomeT> {
    pub ribosome: Ribosome,
    pub invocation: ZomeInvocation,
}

impl<'env, Ribosome> Workflow<'env> for InvokeZomeWorkflow<Ribosome>
where
    Ribosome: RibosomeT + Send + Sync + 'env,
{
    type Output = ZomeInvocationResult;
    type Workspace = InvokeZomeWorkspace<'env>;
    type Triggers = Option<InitializeZomesWorkflow>;

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
            let chain_head_start = workspace.source_chain.chain_head()?.clone();

            let agent_key = invocation.provenance.clone();

            tracing::trace!(line = line!());
            // Create the unsafe sourcechain for use with wasm closure
            let result = {
                let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace);
                ribosome.call_zome_function(raw_workspace, invocation)
            };
            tracing::trace!(line = line!());

            // Get the new head
            let chain_head_end = workspace.source_chain.chain_head()?;

            // Has there been changes?
            if chain_head_start != *chain_head_end {
                // get the changes
                let mut new_headers = workspace
                    .source_chain
                    .iter_back()
                    .scan(None, |current_header, element| {
                        let my_header = current_header.clone();
                        *current_header = element.header().prev_header().cloned();
                        let r = match my_header {
                            Some(current_header) if current_header == chain_head_start => None,
                            _ => Some(element),
                        };
                        Ok(r)
                    })
                    .map_err(WorkflowError::from);

                while let Some(header) = new_headers.next()? {
                    let chain_element = workspace
                        .source_chain
                        .get_element(header.header_address())
                        .await?;
                    let prev_chain_element = chain_element.as_ref().and_then(|c| {
                        let source_chain = &workspace.source_chain;
                        c.header()
                            .prev_header()
                            .cloned()
                            .map(|h| async move { source_chain.get_element(&h).await })
                    });
                    let prev_chain_element =
                        futures::future::OptionFuture::from(prev_chain_element)
                            .await
                            .transpose()?
                            .flatten();
                    if let Some(ref chain_element) = chain_element {
                        sys_validate_element(
                            &agent_key,
                            chain_element,
                            prev_chain_element.as_ref(),
                        )
                        .await?;
                    }
                }
            }

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

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::core::ribosome::wasm_test::zome_invocation_from_names;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::workflow::{effects::WorkflowTriggers, fake_genesis, WorkflowError};
    use holochain_serialized_bytes::prelude::*;
    use holochain_state::{env::ReadManager, test_utils::test_cell_env};
    use holochain_types::{
        entry::Entry, nucleus::ZomeInvocationResponse, observability,
        test_utils::fake_agent_pubkey_1,
    };
    use holochain_zome_types::ZomeExternGuestOutput;

    use futures::{future::BoxFuture, FutureExt};
    use matches::assert_matches;

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct Payload {
        a: u32,
    }

    async fn run_invoke_zome<'env, Ribosome: RibosomeT + Send + Sync + 'env>(
        workspace: InvokeZomeWorkspace<'env>,
        ribosome: Ribosome,
        invocation: ZomeInvocation,
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
            .returning(move |_workspace, _invocation| {
                let x = SerializedBytes::try_from(Payload { a: 3 }).unwrap();
                Ok(ZomeInvocationResponse::ZomeApiFn(
                    ZomeExternGuestOutput::new(x),
                ))
            });

        // Call the zome function
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
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
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
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
    // FIXME: Maybe this should be removed because there it's a way to test that sys validate
    // is called and all the other checks would be duplicating the unit test inside sys val.
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
        let agent_entry = Entry::Agent(agent_pubkey.clone());
        let mut ribosome = MockRibosomeT::new();
        // Call zome mock that it writes to source chain
        ribosome
            .expect_call_zome_function()
            .returning(move |_unsafe_workspace, _invocation| {
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
                Ok(ZomeInvocationResponse::ZomeApiFn(
                    ZomeExternGuestOutput::new(x),
                ))
            });

        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
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
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
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
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
        let (_result, effects) = run_invoke_zome(workspace, ribosome, invocation)
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(effects.signals.is_empty());
        // TODO: Check the workspace has changes
    }
}
