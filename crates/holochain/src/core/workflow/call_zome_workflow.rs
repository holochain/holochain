use super::app_validation_workflow;
use super::error::WorkflowResult;
use super::sys_validation_workflow::sys_validate_element;
use crate::conductor::api::CellConductorApiT;
use crate::conductor::interface::SignalBroadcaster;
use crate::core::queue_consumer::OneshotWriter;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::error::RibosomeError;
use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::RibosomeT;
use crate::core::ribosome::ZomeCallHostAccess;
use crate::core::ribosome::ZomeCallInvocation;
pub use call_zome_workspace_lock::CallZomeWorkspaceLock;
use either::Either;
use holochain_cascade::Cascade;
use holochain_cascade::DbPair;
use holochain_cascade::DbPairMut;
use holochain_keystore::KeystoreSender;
use holochain_p2p::HolochainP2pCell;
use holochain_sqlite::prelude::*;
use holochain_state::element_buf::ElementBuf;
use holochain_state::metadata::MetadataBuf;
use holochain_state::metadata::MetadataBufT;
use holochain_state::source_chain::SourceChain;
use holochain_state::source_chain::SourceChainError;
use holochain_state::workspace::Workspace;
use holochain_state::workspace::WorkspaceResult;
use holochain_zome_types::element::Element;

use holochain_types::prelude::*;
use std::sync::Arc;
use tracing::instrument;

pub mod call_zome_workspace_lock;

#[cfg(test)]
mod validation_test;

/// Placeholder for the return value of a zome invocation
pub type ZomeCallResult = RibosomeResult<ZomeCallResponse>;

#[derive(Debug)]
pub struct CallZomeWorkflowArgs<Ribosome: RibosomeT + Send, C: CellConductorApiT> {
    pub ribosome: Ribosome,
    pub invocation: ZomeCallInvocation,
    pub signal_tx: SignalBroadcaster,
    pub conductor_api: C,
    pub is_root_zome_call: bool,
}

#[instrument(skip(
    workspace_lock,
    network,
    keystore,
    writer,
    args,
    trigger_produce_dht_ops
))]
pub async fn call_zome_workflow<
    'env,
    Ribosome: RibosomeT + Send + 'static,
    C: CellConductorApiT,
>(
    workspace_lock: CallZomeWorkspaceLock,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    writer: OneshotWriter,
    args: CallZomeWorkflowArgs<Ribosome, C>,
    mut trigger_produce_dht_ops: TriggerSender,
) -> WorkflowResult<ZomeCallResult> {
    let should_write = args.is_root_zome_call;
    let result = call_zome_workflow_inner(workspace_lock.clone(), network, keystore, args).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    if should_write {
        let mut guard = workspace_lock.write().await;
        let workspace = &mut guard;
        writer.with_writer(|writer| Ok(workspace.flush_to_txn_ref(writer)?))?;
    }

    trigger_produce_dht_ops.trigger();

    Ok(result)
}

async fn call_zome_workflow_inner<
    'env,
    Ribosome: RibosomeT + Send + 'static,
    C: CellConductorApiT,
>(
    workspace_lock: CallZomeWorkspaceLock,
    network: HolochainP2pCell,
    keystore: KeystoreSender,
    args: CallZomeWorkflowArgs<Ribosome, C>,
) -> WorkflowResult<ZomeCallResult> {
    let CallZomeWorkflowArgs {
        ribosome,
        invocation,
        signal_tx,
        conductor_api,
        ..
    } = args;

    let call_zome_handle = conductor_api.clone().into_call_zome_handle();
    let zome = invocation.zome.clone();

    // Get the current head
    let chain_head_start_len = workspace_lock.read().await.source_chain.len();

    tracing::trace!(line = line!());
    // Create the unsafe sourcechain for use with wasm closure
    let (ribosome, result) = tokio::task::spawn_blocking({
        let workspace_lock = workspace_lock.clone();
        let network = network.clone();
        move || {
            let host_access = ZomeCallHostAccess::new(
                workspace_lock,
                keystore,
                network,
                signal_tx,
                call_zome_handle,
                invocation.cell_id.clone(),
            );
            let result = ribosome.call_zome_function(host_access, invocation);
            (ribosome, result)
        }
    })
    .await?;
    tracing::trace!(line = line!());

    let to_app_validate = {
        let mut workspace = workspace_lock.write().await;
        // Get the new head
        let chain_head_end_len = workspace.source_chain.len();
        let new_elements_len = chain_head_end_len - chain_head_start_len;

        // collect all the elements we need to validate in wasm
        let mut to_app_validate: Vec<Element> = Vec::with_capacity(new_elements_len);

        // Has there been changes?
        if new_elements_len > 0 {
            // Loop forwards through all the new elements
            let mut i = chain_head_start_len;
            while let Some(element) = workspace.source_chain.get_at_index(i as u32)? {
                sys_validate_element(&element, &mut workspace, network.clone(), &conductor_api)
                    .await
                    // If the was en error exit
                    // If the validation failed, exit with an InvalidCommit
                    // If it was ok continue
                    .or_else(|outcome_or_err| outcome_or_err.invalid_call_zome_commit())?;
                to_app_validate.push(element);
                i += 1;
            }
        }
        to_app_validate
    };

    {
        for chain_element in to_app_validate {
            let outcome = match chain_element.header() {
                Header::Dna(_)
                | Header::AgentValidationPkg(_)
                | Header::OpenChain(_)
                | Header::CloseChain(_)
                | Header::InitZomesComplete(_) => {
                    // These headers don't get validated
                    continue;
                }
                Header::CreateLink(link_add) => {
                    let (base, target) = {
                        let mut workspace = workspace_lock.write().await;
                        let mut cascade = workspace.cascade(network.clone());
                        let base_address = &link_add.base_address;
                        let base = cascade
                            .retrieve_entry(base_address.clone(), Default::default())
                            .await
                            .map_err(RibosomeError::from)?
                            .ok_or_else(|| RibosomeError::ElementDeps(base_address.clone().into()))?
                            .into_content();
                        let base = Arc::new(base);

                        let target_address = &link_add.target_address;
                        let target = cascade
                            .retrieve_entry(target_address.clone(), Default::default())
                            .await
                            .map_err(RibosomeError::from)?
                            .ok_or_else(|| {
                                RibosomeError::ElementDeps(target_address.clone().into())
                            })?
                            .into_content();
                        let target = Arc::new(target);
                        (base, target)
                    };
                    let link_add = Arc::new(link_add.clone());
                    Either::Left(
                        app_validation_workflow::run_create_link_validation_callback(
                            zome.clone(),
                            link_add,
                            base,
                            target,
                            &ribosome,
                            workspace_lock.clone(),
                            network.clone(),
                        )?,
                    )
                }
                Header::DeleteLink(delete_link) => Either::Left(
                    app_validation_workflow::run_delete_link_validation_callback(
                        zome.clone(),
                        delete_link.clone(),
                        &ribosome,
                        workspace_lock.clone(),
                        network.clone(),
                    )?,
                ),
                Header::Create(_) | Header::Update(_) | Header::Delete(_) => Either::Right(
                    app_validation_workflow::run_validation_callback_direct(
                        zome.clone(),
                        chain_element,
                        &ribosome,
                        workspace_lock.clone(),
                        network.clone(),
                        &conductor_api,
                    )
                    .await?,
                ),
            };
            match outcome {
                Either::Left(outcome) => match outcome {
                    app_validation_workflow::Outcome::Accepted => {}
                    app_validation_workflow::Outcome::Rejected(reason) => {
                        return Err(SourceChainError::InvalidLink(reason).into());
                    }
                    app_validation_workflow::Outcome::AwaitingDeps(hashes) => {
                        return Err(SourceChainError::InvalidCommit(format!("{:?}", hashes)).into());
                    }
                },
                Either::Right(outcome) => match outcome {
                    app_validation_workflow::Outcome::Accepted => {}
                    app_validation_workflow::Outcome::Rejected(reason) => {
                        return Err(SourceChainError::InvalidCommit(reason).into());
                    }
                    // when the wasm is being called directly in a zome invocation any
                    // state other than valid is not allowed for new entries
                    // e.g. we require that all dependencies are met when committing an
                    // entry to a local source chain
                    // this is different to the case where we are validating data coming in
                    // from the network where unmet dependencies would need to be
                    // rescheduled to attempt later due to partitions etc.
                    app_validation_workflow::Outcome::AwaitingDeps(hashes) => {
                        return Err(SourceChainError::InvalidCommit(format!("{:?}", hashes)).into());
                    }
                },
            }
        }
    }

    Ok(result)
}

pub struct CallZomeWorkspace {
    pub source_chain: SourceChain,
    pub meta_authored: MetadataBuf<AuthoredPrefix>,
    pub element_integrated: ElementBuf<IntegratedPrefix>,
    pub meta_integrated: MetadataBuf<IntegratedPrefix>,
    pub element_rejected: ElementBuf<RejectedPrefix>,
    pub meta_rejected: MetadataBuf<RejectedPrefix>,
    pub element_cache: ElementBuf,
    pub meta_cache: MetadataBuf,
}

impl<'a> CallZomeWorkspace {
    pub fn new(env: DbRead) -> WorkspaceResult<Self> {
        let source_chain = SourceChain::new(env.clone())?;
        let meta_authored = MetadataBuf::authored(env.clone())?;
        let element_integrated = ElementBuf::vault(env.clone(), true)?;
        let meta_integrated = MetadataBuf::vault(env.clone())?;
        let element_rejected = ElementBuf::rejected(env.clone())?;
        let meta_rejected = MetadataBuf::rejected(env.clone())?;
        let element_cache = ElementBuf::cache(env.clone())?;
        let meta_cache = MetadataBuf::cache(env)?;

        Ok(CallZomeWorkspace {
            source_chain,
            meta_authored,
            element_integrated,
            meta_integrated,
            element_rejected,
            meta_rejected,
            element_cache,
            meta_cache,
        })
    }

    pub fn cascade(&'a mut self, network: HolochainP2pCell) -> Cascade<'a> {
        Cascade::new(
            self.source_chain.env().clone(),
            &self.source_chain.elements(),
            &self.meta_authored,
            &self.element_integrated,
            &self.meta_integrated,
            &self.element_rejected,
            &self.meta_rejected,
            &mut self.element_cache,
            &mut self.meta_cache,
            network,
        )
    }

    /// Cascade without a network connection
    pub fn cascade_local(&'a mut self) -> Cascade<'a> {
        let authored_data = DbPair::new(&self.source_chain.elements(), &self.meta_authored);
        let cache_data = DbPairMut::new(&mut self.element_cache, &mut self.meta_cache);
        let integrated_data = DbPair::new(&self.element_integrated, &self.meta_integrated);
        Cascade::empty()
            .with_authored(authored_data)
            .with_cache(cache_data)
            .with_integrated(integrated_data)
    }

    pub fn env(&self) -> &DbRead {
        self.meta_authored.env()
    }
}

impl Workspace for CallZomeWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn_ref(writer)?;
        self.meta_authored.flush_to_txn_ref(writer)?;
        self.element_cache.flush_to_txn_ref(writer)?;
        self.meta_cache.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::conductor::api::CellConductorApi;
    use crate::conductor::handle::MockConductorHandleT;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::workflow::error::WorkflowError;
    use crate::core::workflow::genesis_workflow::tests::fake_genesis;
    use crate::fixt::*;
    use ::fixt::prelude::*;

    use holochain_p2p::HolochainP2pCellFixturator;
    use holochain_sqlite::db::ReadManager;
    use holochain_sqlite::test_utils::test_cell_env;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_wasm_test_utils::TestWasm;
    use holochain_zome_types::cell::CellId;
    use holochain_zome_types::entry::Entry;
    use holochain_zome_types::ExternIO;
    use matches::assert_matches;
    use observability;

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct Payload {
        a: u32,
    }

    async fn run_call_zome<'env, Ribosome: RibosomeT + Send + Sync + 'static>(
        workspace: CallZomeWorkspace,
        ribosome: Ribosome,
        invocation: ZomeCallInvocation,
    ) -> WorkflowResult<ZomeCallResult> {
        let keystore = fixt!(KeystoreSender);
        let network = fixt!(HolochainP2pCell);
        let cell_id = CellId::new(ribosome.dna_def().as_hash().clone(), fixt!(AgentPubKey));
        let conductor_api = Arc::new(MockConductorHandleT::new());
        let conductor_api = CellConductorApi::new(conductor_api, cell_id);
        let args = CallZomeWorkflowArgs {
            invocation,
            ribosome,
            signal_tx: SignalBroadcaster::noop(),
            conductor_api,
            is_root_zome_call: true,
        };
        call_zome_workflow_inner(workspace.into(), network, keystore, args).await
    }

    // 1.  Check if there is a Capability token secret in the parameters.
    // If there isn't and the function to be called isn't public,
    // we stop the process and return an error. MVT
    #[ignore = "TODO: B-01553: Finish this test when capabilities land"]
    #[allow(unused_variables, unreachable_code)]
    #[tokio::test]
    async fn private_zome_call() {
        let test_env = test_cell_env();
        let env = test_env.env();
        let mut g = env.guard();
        let reader = g.reader().unwrap();
        let workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();
        let ribosome = MockRibosomeT::new();
        // FIXME: CAP: Set this function to private
        let invocation =
            crate::fixt::ZomeCallInvocationFixturator::new(crate::fixt::NamedInvocation(
                holochain_types::fixt::CellIdFixturator::new(::fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                ExternIO::encode(Payload { a: 1 }).unwrap(),
            ))
            .next()
            .unwrap();
        invocation.cap = todo!("Make secret cap token");
        let error = run_call_zome(workspace, ribosome, invocation)
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
    #[ignore = "TODO: B-01100 Make sure this test is in the right place when SysValidation
    complete so we aren't duplicating the unit test inside sys val."]
    #[tokio::test]
    async fn calls_system_validation<'a>() {
        observability::test_run().ok();
        let test_env = test_cell_env();
        let env = test_env.env();
        let mut workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();

        // Genesis
        fake_genesis(&mut workspace.source_chain).await.unwrap();

        let agent_pubkey = fake_agent_pubkey_1();
        let _agent_entry = Entry::Agent(agent_pubkey.clone().into());
        let mut ribosome = MockRibosomeT::new();
        // Call zome mock that it writes to source chain
        ribosome
            .expect_call_zome_function()
            .returning(move |_workspace, _invocation| {
                Ok(ZomeCallResponse::Ok(
                    ExternIO::encode(Payload { a: 3 }).unwrap(),
                ))
            });

        let invocation =
            crate::fixt::ZomeCallInvocationFixturator::new(crate::fixt::NamedInvocation(
                holochain_types::fixt::CellIdFixturator::new(::fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                ExternIO::encode(Payload { a: 1 }).unwrap(),
            ))
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

        let _result = run_call_zome(workspace, ribosome, invocation)
            .await
            .unwrap();
    }

    // 4.2. Call app validation of list of entries and headers: (MVI)
    // - Call validate_set_of_entries_and_headers (any necessary get
    //   results where we receive None / Timeout on retrieving validation dependencies, should produce error/fail)
    #[ignore = "TODO: B-01093: Finish when app val lands"]
    #[tokio::test]
    async fn calls_app_validation() {
        let test_env = test_cell_env();
        let env = test_env.env();
        let workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();
        let ribosome = MockRibosomeT::new();
        let invocation =
            crate::fixt::ZomeCallInvocationFixturator::new(crate::fixt::NamedInvocation(
                holochain_types::fixt::CellIdFixturator::new(::fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                ExternIO::encode(Payload { a: 1 }).unwrap(),
            ))
            .next()
            .unwrap();
        // TODO: B-01093: Mock the app validation and check it's called
        // TODO: B-01093: How can I pass a app validation into this?
        // These are just static calls
        let _result = run_call_zome(workspace, ribosome, invocation)
            .await
            .unwrap();
    }

    // 4.3. Write output results via SC gatekeeper (wrap in transaction): (MVI)
    // This is handled by the workflow runner however I should test that
    // we can create outputs
    #[tokio::test(threaded_scheduler)]
    async fn creates_outputs() {
        let test_env = test_cell_env();
        let env = test_env.env();
        let workspace = CallZomeWorkspace::new(env.clone().into()).unwrap();
        let mut ribosome = MockRibosomeT::new();
        let dna_def = fixt!(DnaFile).dna().clone();
        ribosome.expect_dna_def().return_const(dna_def);
        ribosome
            .expect_call_zome_function()
            .returning(|_, _| Ok(ZomeCallResponse::Ok(ExternIO::encode(()).unwrap())));
        // TODO: Make this mock return an output
        let invocation =
            crate::fixt::ZomeCallInvocationFixturator::new(crate::fixt::NamedInvocation(
                holochain_types::fixt::CellIdFixturator::new(::fixt::Unpredictable)
                    .next()
                    .unwrap(),
                TestWasm::Foo.into(),
                "fun_times".into(),
                ExternIO::encode(Payload { a: 1 }).unwrap(),
            ))
            .next()
            .unwrap();
        let _result = run_call_zome(workspace, ribosome, invocation)
            .await
            .unwrap();
        // TODO: Check the workspace has changes
    }
}
