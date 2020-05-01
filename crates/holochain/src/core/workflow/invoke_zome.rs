use super::{system_validation::SystemValidation, WorkflowEffects, WorkflowError, WorkflowResult};
use crate::core::{
    ribosome::RibosomeT,
    state::{source_chain::UnsafeSourceChain, workspace::InvokeZomeWorkspace},
};
use fallible_iterator::FallibleIterator;
use holochain_types::nucleus::ZomeInvocation;

pub async fn invoke_zome<'env>(
    mut workspace: InvokeZomeWorkspace<'_>,
    ribosome: impl RibosomeT,
    invocation: ZomeInvocation,
    sv: impl SystemValidation,
) -> WorkflowResult<InvokeZomeWorkspace<'_>> {
    let chain_head_start = workspace.chain_head()?.clone();
    {
        let (_g, source_chain) = UnsafeSourceChain::from_mut(&mut workspace.source_chain);
        let _result = ribosome.call_zome_function(source_chain, invocation)?;
    }
    let chain_head_end = workspace.chain_head()?;
    // Has there been changes?
    if chain_head_start != *chain_head_end {
        // get the changes
        workspace
            .iter_back()
            .scan(None, |current_header, entry| {
                let my_header = current_header.clone();
                *current_header = entry.prev_header_address.clone();
                let r = match my_header {
                    Some(current_header) if current_header == chain_head_start => None,
                    _ => Some(entry),
                };
                Ok(r)
            })
            .map_err(|e| WorkflowError::from(e))
            // call the sys validation on the changes
            .map(|chain_head| Ok(sv.check_entry_hash(&chain_head.entry_address.into())?))
            .collect::<Vec<_>>()?;
    }

    Ok(WorkflowEffects {
        workspace,
        triggers: Default::default(),
        signals: Default::default(),
        callbacks: Default::default(),
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::core::ribosome::wasm_test::zome_invocation_from_names;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::{
        state::source_chain::SourceChain,
        workflow::{WorkflowCall, WorkflowError},
    };
    use holochain_serialized_bytes::prelude::*;
    use holochain_state::{env::ReadManager, prelude::Reader, test_utils::test_cell_env};
    use holochain_types::{
        entry::Entry,
        nucleus::ZomeInvocationResponse,
        observability,
        test_utils::{fake_agent_hash, fake_dna},
    };
    use holochain_zome_types::ZomeExternGuestOutput;

    use crate::core::workflow::system_validation::{MockSystemValidation, PlaceholderSysVal};
    use matches::assert_matches;

    #[derive(Debug, serde::Serialize, serde::Deserialize, SerializedBytes)]
    struct Payload {
        a: u32,
    }

    #[tokio::test]
    async fn runs_init() {
        let env = test_cell_env();
        let dbs = env.dbs().await.unwrap();
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let ribosome = MockRibosomeT::new();
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
        let effects = invoke_zome(workspace, ribosome, invocation, PlaceholderSysVal {})
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert_matches!(effects.triggers[0].interval, None);
        assert_matches!(effects.triggers[0].call, WorkflowCall::InitializeZome);
    }

    // 1.  Check if there is a Capability token secret in the parameters.
    // If there isn't and the function to be called isn't public,
    // we stop the process and return an error. MVT
    // TODO: Finish this test when capabilities land
    #[ignore]
    #[allow(unused_variables, unreachable_code)]
    #[tokio::test]
    async fn private_zome_call() {
        let env = test_cell_env();
        let dbs = env.dbs().await.unwrap();
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let ribosome = MockRibosomeT::new();
        // FIXME: CAP: Set this function to private
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
        invocation.cap = todo!("Make secret cap token");
        let error = invoke_zome(workspace, ribosome, invocation, PlaceholderSysVal {})
            .await
            .unwrap_err();
        assert_matches!(error, WorkflowError::CapabilityMissing);
    }

    // TODO: Finish these tests when capabilities land
    // 1.1 If there is a secret, we look up our private CAS and see if it matches any secret for a
    // Capability Grant entry that we have stored. If it does, check that this Capability Grant is
    //not revoked and actually grants permissions to call the ZomeFn that is being called. (MVI)

    // 1.2 Check if the Capability Grant has assignees=None (means this Capability is transferable).
    // If it has assignees=Vec<Address> (means this Capability is on Assigned mode, check that the
    // provenance's agent key is in that assignees. (MVI)

    // 1.3 If the CapabiltyGrant has pre-filled parameters, check that the ui is passing exactly the
    // parameters needed and no more to complete the call. (MVI)

    // TODO: What is pre-flight cain extention?
    // 2. Set Context (Cascading Cursor w/ Pre-flight chain extension) MVT

    // TODO: How is the Cursor (I guess the cascade?) passed to the wasm invokation?
    // Might just be inside the ribosome?
    // 3. Invoke WASM (w/ Cursor) MVM
    // WASM receives external call handles:
    // (gets & commits via cascading cursor, crypto functions & bridge calls via conductor,
    // send via network function call for send direct message)

    // 4. When the WASM code execution finishes, If workspace has new chain entries:
    // 4.1. Call system validation of list of entries and headers: (MVI)
    // - Check entry hash
    // - Check header hash
    // - Check header signature
    // - Check header timestamp is later than previous timestamp
    // - Check entry content matches entry schema
    //   Depending on the type of the commit, validate all possible validations for the
    //   DHT Op that would be produced by it
    #[tokio::test]
    async fn calls_system_validation() {
        observability::test_run().ok();
        let env = test_cell_env();
        let dbs = env.dbs().await.unwrap();
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();

        // Genesis
        let agent_hash = fake_agent_hash("cool agent");
        let agent_entry = Entry::AgentKey(agent_hash.clone());
        let dna_entry = Entry::Dna(Box::new(fake_dna("cool dna")));
        workspace.put_entry(agent_entry, &agent_hash).unwrap();
        workspace.put_entry(dna_entry, &agent_hash).unwrap();

        let mut ribosome = MockRibosomeT::new();
        // Call zome mock that it writes to source chain
        ribosome
            .expect_call_zome_function()
            .returning(move |source_chain, _invocation| {
                let agent_entry = Entry::AgentKey(agent_hash.clone());
                let call = |source_chain: &mut SourceChain<Reader>| {
                    source_chain.put_entry(agent_entry, &agent_hash).unwrap()
                };
                unsafe { source_chain.apply_mut(call) };
                let x = SerializedBytes::try_from(Payload { a: 3 }).unwrap();
                Ok(ZomeInvocationResponse::ZomeApiFn(
                    ZomeExternGuestOutput::new(x),
                ))
            });
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
        // TODO: Mock the system validation and check it's called
        let mut sys_val = MockSystemValidation::new();
        sys_val
            .expect_check_entry_hash()
            .times(1)
            .returning(|_entry_hash| Ok(()));

        let effects = invoke_zome(workspace, ribosome, invocation, sys_val)
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(effects.signals.is_empty());
    }

    // 4.2. Call app validation of list of entries and headers: (MVI)
    // - Call validate_set_of_entries_and_headers (any necessary get
    //   results where we receive None / Timeout on retrieving validation dependencies, should produce error/fail)
    #[tokio::test]
    async fn calls_app_validation() {
        let env = test_cell_env();
        let dbs = env.dbs().await.unwrap();
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let ribosome = MockRibosomeT::new();
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
        // TODO: Mock the app validation and check it's called
        // TODO: How can I pass a app validation into this?
        // These are just static calls
        let effects = invoke_zome(workspace, ribosome, invocation, PlaceholderSysVal {})
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(effects.signals.is_empty());
    }

    // 4.3. Write output results via SC gatekeeper (wrap in transaction): (MVI)
    // This is handled by the workflow runner however I should test that
    // we can create outputs
    #[tokio::test]
    async fn creates_outputs() {
        let env = test_cell_env();
        let dbs = env.dbs().await.unwrap();
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let workspace = InvokeZomeWorkspace::new(&reader, &dbs).unwrap();
        let ribosome = MockRibosomeT::new();
        // TODO: Make this mock return an output
        let invocation =
            zome_invocation_from_names("zomey", "fun_times", Payload { a: 1 }.try_into().unwrap());
        let effects = invoke_zome(workspace, ribosome, invocation, PlaceholderSysVal {})
            .await
            .unwrap();
        assert!(effects.triggers.is_empty());
        assert!(effects.callbacks.is_empty());
        assert!(effects.signals.is_empty());
        // TODO: Check the workspace has changes
    }

    #[cfg(test_TODO_FIX)]
    #[tokio::test]
    async fn can_invoke_zome_with_mock() {
        let cell_id = fake_cell_id("mario");
        let tmpdir = TempDir::new("holochain_2020").unwrap();
        let persistence = SourceChainPersistence::test(tmpdir.path());
        let chain = test_initialized_chain("mario", &persistence);
        let invocation = ZomeInvocation {
            cell_id: cell_id.clone(),
            zome_name: "zome".into(),
            fn_name: "fn".into(),
            as_at: "KwyXHisn".into(),
            args: "args".into(),
            provenance: cell_id.agent_id().to_owned(),
            cap: CapabilityRequest,
        };

        let mut ribosome = MockRibosomeT::new();
        ribosome
            .expect_call_zome_function()
            .times(1)
            .returning(|bundle, _| Ok(ZomeInvocationResponse));

        // TODO: make actual assertions on the conductor_api, once more of the
        // actual logic is fleshed out
        let mut conductor_api = MockCellConductorApi::new();

        let result = invoke_zome(invocation, chain, ribosome, conductor_api).await;
        assert!(result.is_ok());
    }

    // TODO: can try making a fake (not mock) ribosome that has some hard-coded logic
    // for calling into a ZomeApi, rather than needing to write a test DNA. This will
    // have to wait until the whole WasmRibosome thing is more fleshed out.
    // struct FakeRibosome;

    // impl RibosomeT for FakeRibosome {
    //     fn run_validation(self, cursor: &source_chain::Cursor, entry: Entry) -> ValidationResult {
    //         unimplemented!()
    //     }

    //     /// Runs the specified zome fn. Returns the cursor used by HDK,
    //     /// so that it can be passed on to source chain manager for transactional writes
    //     fn call_zome_function(
    //         self,
    //         bundle: SourceChainCommitBundle,
    //         invocation: ZomeInvocation,
    //     ) -> SkunkResult<(ZomeInvocationResponse, SourceChainCommitBundle)> {
    //         unimplemented!()
    //     }
    // }
}
